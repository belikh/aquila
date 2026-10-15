//! `aquila-addon` — the composition root (`architecture.md` §9).
//!
//! This binary holds **no domain logic**: it loads the topology, constructs the
//! adapters, wires them into `aquila-sensing`, and starts the private HTTP
//! control plane (`aquila-api`). It is unlayered and exempt from the dependency
//! level rule.
//!
//! # Running as a Home Assistant add-on
//!
//! The container must stay up and serve until Home Assistant stops it
//! (`home-assistant.md`). Concretely the root:
//!
//! - binds `0.0.0.0:<port>` by default, so the container address is reachable
//!   through Home Assistant ingress (a loopback bind is invisible to the
//!   ingress proxy); `--host`/`AQUILA_BIND_ADDR` restore a loopback bind for
//!   local development;
//! - serves the pre-built WASM dashboard bundle from `AQUILA_DASHBOARD_DIR` on
//!   the same listener as `/api/v1`, honouring the ingress path prefix
//!   (`dashboard.md` §4);
//! - drives the sensing pipeline in a loop that never exits at end-of-source:
//!   the deterministic scene repeats (or a live UDP source is used when
//!   `AQUILA_LIVE_ADDR` is set), so the process serves until stopped;
//! - calibrates on startup from a synthetic empty-home baseline plus
//!   walk-through captures, so the calibration-gated core emits real
//!   presence/count/vitals events the dashboard and the Home Assistant
//!   component can read; and
//! - installs a safe SIGTERM/SIGINT handler (via `sigwait` in a dedicated
//!   thread, no `unsafe`) so Home Assistant's `SIGTERM` is a clean shutdown:
//!   flush the sinks, stop the listener, exit zero.
//!
//! A production build swaps the simulator for `UdpLiveSource` and supplies real
//! models; the wiring shape is identical.
//!
//! # Prose
//!
//! Australian English throughout (serialisation, behaviour, normalise).

#![forbid(unsafe_code)]

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aquila_api::{Auth, ChannelSink, Server, ServerConfig, StaticAssets};
use aquila_geometry::Topology;
use aquila_metrics::InMemoryMetricSink;
use aquila_sensing::{CalibrationInputs, CalibrationRequest, Sensing, SensingConfig, SystemClock};
use aquila_simulator::{
    Actor, Impairments, Motion, NodeSpec, Rhythm, SceneScript, SimConfig, SimulatorSource,
};
use aquila_sources::{CsiSource, FrameOrEnd, SourceError};
use aquila_types::{
    Band, Clock, CsiEnvelope, DeviceType, FirmwareRef, Floor, Home, MacAddr, Node, NodeId, Room,
    RoomId, RoomKind, Slug, Topology as ContractTopology, CONTRACT_VERSION,
};
use aquila_wire::{decode_datagram, MAX_DATAGRAM_LEN};

/// The default control-plane port (`core-api.md` §2).
const DEFAULT_PORT: u16 = 8099;

/// The default bind address: every interface, so the container is reachable
/// through Home Assistant ingress.
const DEFAULT_HOST: IpAddr = IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED);

/// The environment variable naming the baked dashboard bundle directory.
const DASHBOARD_DIR_ENV: &str = aquila_api::DASHBOARD_DIR_ENV;

/// When set, the composition root pulls live CSI from this UDP address instead
/// of repeating the deterministic simulator scene.
const LIVE_ADDR_ENV: &str = "AQUILA_LIVE_ADDR";

/// The deterministic RNG seed for the demo scene and its calibration captures.
const DEMO_SEED: u64 = 2026;

/// A fixed, deterministic generation stamp for the startup calibration. The
/// wizard reads no wall clock, so the artefact is reproducible run to run.
const CALIBRATION_GENERATED_AT: &str = "2026-01-01T00:00:00Z";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args) {
        Command::Help => print_help(),
        Command::Run(options) => {
            if let Err(error) = run(&options) {
                eprintln!("aquila-addon: {error}");
                std::process::exit(1);
            }
        }
        Command::Error(message) => {
            eprintln!("aquila-addon: {message}\n");
            print_help();
            std::process::exit(2);
        }
    }
}

/// The parsed command line.
enum Command {
    Help,
    Run(Options),
    Error(String),
}

/// The default-run options.
struct Options {
    host: IpAddr,
    port: u16,
}

/// Parse the minimal CLI: `--help`, an optional `--host` and an optional
/// `--port`, each defaulting from the environment.
fn parse(args: &[String]) -> Command {
    let mut port = std::env::var("AQUILA_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    let mut host = std::env::var("AQUILA_BIND_ADDR")
        .ok()
        .and_then(|value| value.parse::<IpAddr>().ok())
        .unwrap_or(DEFAULT_HOST);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Command::Help,
            "--port" => {
                let Some(value) = args.get(index + 1) else {
                    return Command::Error("--port requires a value".to_string());
                };
                match value.parse::<u16>() {
                    Ok(parsed) => port = parsed,
                    Err(_) => return Command::Error(format!("invalid port `{value}`")),
                }
                index += 1;
            }
            "--host" => {
                let Some(value) = args.get(index + 1) else {
                    return Command::Error("--host requires a value".to_string());
                };
                match value.parse::<IpAddr>() {
                    Ok(parsed) => host = parsed,
                    Err(_) => return Command::Error(format!("invalid host `{value}`")),
                }
                index += 1;
            }
            other => return Command::Error(format!("unknown argument `{other}`")),
        }
        index += 1;
    }
    Command::Run(Options { host, port })
}

/// Wire the pipeline and run it until stopped.
fn run(options: &Options) -> Result<(), String> {
    // Provenance is required: the ledger is commit-keyed, so the composition
    // root supplies the real build commit rather than letting the core fabricate
    // a zero hash (`sister-issues-core.md` S15). The build/release tooling sets
    // `AQUILA_COMMIT`; refusing to start without it is the honest default.
    let commit = std::env::var("AQUILA_COMMIT").map_err(|_| {
        "AQUILA_COMMIT is not set: the sensing core will not key its ledger to an \
         unknown commit"
            .to_string()
    })?;

    let script = demo_scene();
    let topology = topology_for_scene(&script);
    let mut sensing_config = SensingConfig::default().with_commit(commit.clone());
    // The add-on surfaces vitals to Home Assistant (`home-assistant.md` §6.1), so
    // the core estimates breathing and heart rate alongside presence and count.
    sensing_config.vitals_enabled = true;

    // Block SIGTERM/SIGINT before any worker thread exists, so every thread
    // inherits the mask and only the dedicated `sigwait` thread observes them.
    let shutdown = install_signal_handling()?;

    let addr = SocketAddr::new(options.host, options.port);
    let assets = StaticAssets::from_env();
    let server = Server::bind_with_config(
        addr,
        &topology,
        "aquila-addon",
        ServerConfig::new()
            .with_auth(Auth::open())
            .with_assets(assets.clone())
            .any_interface(),
    )
    .map_err(|e| format!("cannot bind control plane on {addr}: {e}"))?;
    let bound = server
        .local_addr()
        .map_err(|e| format!("cannot read local address: {e}"))?;
    let handle = server.handle();
    let mut guard = server
        .serve_in_background()
        .map_err(|e| format!("cannot serve control plane: {e}"))?;

    println!("aquila-addon: control plane on http://{bound}/api/v1/health");
    println!("aquila-addon: health  http://{bound}/api/v1/health");
    match assets.as_ref() {
        Some(assets) => println!(
            "aquila-addon: dashboard  http://{bound}/  (from {})",
            assets.root().display()
        ),
        None => println!(
            "aquila-addon: dashboard bundle not configured ({DASHBOARD_DIR_ENV} unset or absent)"
        ),
    }

    let clock: Arc<dyn Clock> = Arc::new(SystemClock::new());
    let (source, pace): (Box<dyn CsiSource>, Option<Duration>) =
        build_source(&script, &shutdown, Arc::clone(&clock))?;
    let sink = Box::new(ChannelSink::new(handle.clone()));
    let metrics = Box::new(InMemoryMetricSink::new());

    let mut sensing = Sensing::new(
        source,
        Arc::clone(&clock),
        sink,
        metrics,
        None,
        None,
        topology.model().clone(),
        sensing_config,
    )
    .map_err(|e| format!("cannot build the sensing core: {e}"))?;

    calibrate_on_startup(&mut sensing, &script, &commit)?;

    // Run persistently: the source never ends (a repeat or a live stream), and
    // the API serves on its background thread throughout. The loop observes the
    // shutdown flag between steps, so SIGTERM/SIGINT ends it cleanly.
    let started = Instant::now();
    let mut last_status = Instant::now();
    while !shutdown.load(Ordering::Relaxed) {
        let report = sensing.step().map_err(|e| format!("step failed: {e}"))?;
        if report.ended {
            break;
        }
        if let Some(pace) = pace {
            std::thread::sleep(pace);
        }
        if last_status.elapsed() >= Duration::from_secs(30) {
            println!(
                "aquila-addon: {} events published, {} windows processed, up {}s",
                handle.revision(),
                sensing.windows_processed(),
                started.elapsed().as_secs()
            );
            last_status = Instant::now();
        }
    }

    // Clean shutdown: flush the core's sinks, stop the listener, report totals.
    sensing.sink().map_err(|e| format!("flush failed: {e}"))?;
    guard.shutdown();
    println!(
        "aquila-addon: stopped after {} events published, {} windows processed",
        handle.revision(),
        sensing.windows_processed()
    );
    Ok(())
}

/// Calibrate the core from a synthetic empty-home baseline and walk-through
/// captures, so the calibration-gated detector emits real decisions rather than
/// reporting "unknown" (`geometry-calibration.md`).
///
/// The captures are generated deterministically from the same scene layout as
/// the runtime demo, so the artefact binds to the identical topology digest. A
/// failure to calibrate is logged and the core keeps running (uncalibrated),
/// rather than taking the add-on down.
fn calibrate_on_startup(
    sensing: &mut Sensing,
    script: &SceneScript,
    commit: &str,
) -> Result<(), String> {
    let inputs = calibration_inputs(script)?;
    let request = CalibrationRequest::new(commit, "addon-startup", CALIBRATION_GENERATED_AT);
    match sensing.calibrate(&request, &inputs) {
        Ok(report) if report.calibrated => {
            println!(
                "aquila-addon: calibrated ({:?}) — {}",
                report.quality, report.note
            );
            Ok(())
        }
        Ok(report) => {
            eprintln!("aquila-addon: not calibrated — {}", report.note);
            Ok(())
        }
        Err(error) => {
            eprintln!("aquila-addon: calibration failed, continuing uncalibrated: {error}");
            Ok(())
        }
    }
}

/// Build the empty-home and walk-through captures for the startup calibration.
fn calibration_inputs(script: &SceneScript) -> Result<CalibrationInputs, String> {
    let empty = collect_frames(&empty_scene(script))?;
    let mut walkthrough = Vec::new();
    for room in scene_rooms(script) {
        let frames = collect_frames(&walk_scene(script, &room))?;
        walkthrough.push((room, frames));
    }
    Ok(CalibrationInputs::new(empty, walkthrough))
}

/// Drain one finite simulator run into its decoded envelopes.
fn collect_frames(script: &SceneScript) -> Result<Vec<CsiEnvelope>, String> {
    let config = SimConfig::new(DEMO_SEED, Slug(script.name.clone()), 0, 0);
    let mut source = SimulatorSource::new(script.clone(), config)
        .map_err(|e| format!("cannot build simulator: {e}"))?;
    let mut frames = Vec::new();
    while let FrameOrEnd::Frame(envelope) = source
        .recv()
        .map_err(|e| format!("simulator failed: {e}"))?
    {
        frames.push(envelope);
    }
    Ok(frames)
}

/// Build the CSI source for the run: a live UDP stream when `AQUILA_LIVE_ADDR`
/// is set, otherwise the deterministic demo scene repeated forever.
fn build_source(
    script: &SceneScript,
    shutdown: &Arc<AtomicBool>,
    clock: Arc<dyn Clock>,
) -> Result<(Box<dyn CsiSource>, Option<Duration>), String> {
    if let Ok(addr) = std::env::var(LIVE_ADDR_ENV) {
        let addr: SocketAddr = addr
            .parse()
            .map_err(|e| format!("invalid {LIVE_ADDR_ENV} `{addr}`: {e}"))?;
        let socket = UdpSocket::bind(addr)
            .map_err(|e| format!("cannot bind live CSI socket on {addr}: {e}"))?;
        socket
            .set_read_timeout(Some(Duration::from_millis(200)))
            .map_err(|e| format!("cannot configure live socket: {e}"))?;
        println!("aquila-addon: live CSI on udp://{addr}");
        let source = LiveUdpSource {
            socket,
            clock,
            stop: Arc::clone(shutdown),
            buffer: vec![0u8; MAX_DATAGRAM_LEN],
        };
        return Ok((Box::new(source), None));
    }

    // Repeat the scene: the finite simulator's frames are captured once and
    // replayed with a monotonic per-cycle time offset, so the core sees a
    // continuous stream and its windows keep closing. The loop is paced to the
    // scene's frame rate so the add-on does not spin.
    let frames = collect_frames(script)?;
    let frame_period = if frames.is_empty() || script.tick_hz <= 0.0 || script.nodes.is_empty() {
        Duration::from_millis(25)
    } else {
        // One frame arrives per node per tick; pace the loop to that rate so the
        // add-on does not spin (a 20 Hz two-node scene paces at 25 ms/frame).
        let per_frame = 1.0 / (script.tick_hz * script.nodes.len() as f64);
        Duration::from_secs_f64(per_frame.clamp(0.001, 1.0))
    };
    let source = RepeatingSource::new(frames, script.duration_s);
    println!(
        "aquila-addon: repeating demo scene `{}` ({} nodes, {:.0}s/cycle)",
        script.name,
        script.nodes.len(),
        script.duration_s
    );
    Ok((Box::new(source), Some(frame_period)))
}

/// A deterministic simulator scene that is replayed forever.
///
/// The finite [`SimulatorSource`] frames are captured once and re-emitted with a
/// per-cycle offset added to both the host-arrival and device timestamps, so the
/// timeline is monotonic across cycles and the core's clock fit and windows stay
/// coherent.
struct RepeatingSource {
    frames: Vec<CsiEnvelope>,
    cursor: usize,
    cycle: u64,
    host_span_ns: u64,
    device_span_us: u64,
    unix_span_ms: i64,
    stopped: bool,
}

impl RepeatingSource {
    fn new(frames: Vec<CsiEnvelope>, duration_s: f64) -> Self {
        let duration_s = duration_s.max(0.0);
        Self {
            frames,
            cursor: 0,
            cycle: 0,
            host_span_ns: (duration_s * 1e9).round() as u64,
            device_span_us: (duration_s * 1e6).round() as u64,
            unix_span_ms: (duration_s * 1e3).round() as i64,
            stopped: false,
        }
    }
}

impl CsiSource for RepeatingSource {
    fn recv(&mut self) -> Result<FrameOrEnd, SourceError> {
        if self.stopped {
            return Err(SourceError::Stopped);
        }
        if self.frames.is_empty() {
            return Ok(FrameOrEnd::End);
        }
        if self.cursor >= self.frames.len() {
            self.cursor = 0;
            self.cycle = self.cycle.saturating_add(1);
        }
        let mut envelope = self.frames[self.cursor].clone();
        self.cursor += 1;
        let host_offset = self.host_span_ns.saturating_mul(self.cycle);
        envelope.host_rx_mono_ns = envelope.host_rx_mono_ns.saturating_add(host_offset);
        envelope.host_rx_unix_ms = envelope
            .host_rx_unix_ms
            .saturating_add(self.unix_span_ms.saturating_mul(self.cycle as i64));
        envelope.frame.device_time_us = envelope
            .frame
            .device_time_us
            .saturating_add(self.device_span_us.saturating_mul(self.cycle));
        Ok(FrameOrEnd::Frame(envelope))
    }

    fn shutdown(&mut self) {
        self.stopped = true;
    }
}

/// The live UDP CSI adapter (`capture-and-firmware.md` §5), made shutdown-aware.
///
/// It mirrors `aquila_sources::UdpLiveSource` (one socket, demultiplexed by the
/// node id in each datagram, host time from the injected clock) but returns
/// [`FrameOrEnd::End`] once the process shutdown flag is set, so a `SIGTERM`
/// ends the pull loop even when no frames are arriving.
struct LiveUdpSource {
    socket: UdpSocket,
    clock: Arc<dyn Clock>,
    stop: Arc<AtomicBool>,
    buffer: Vec<u8>,
}

impl CsiSource for LiveUdpSource {
    fn recv(&mut self) -> Result<FrameOrEnd, SourceError> {
        loop {
            if self.stop.load(Ordering::Relaxed) {
                return Ok(FrameOrEnd::End);
            }
            match self.socket.recv_from(&mut self.buffer) {
                Ok((len, from)) => match decode_datagram(&self.buffer[..len]) {
                    Ok(frame) => {
                        return Ok(FrameOrEnd::Frame(CsiEnvelope {
                            frame,
                            host_rx_mono_ns: self.clock.now().as_nanos(),
                            host_rx_unix_ms: 0,
                            source_addr: Some(from),
                        }))
                    }
                    Err(_) => continue,
                },
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut
                        || e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(SourceError::Io(e)),
            }
        }
    }

    fn shutdown(&mut self) {}
}

/// Install a safe SIGTERM/SIGINT handler.
///
/// The signals are blocked on the calling thread (and inherited by every thread
/// it later spawns); a dedicated thread waits for one with `sigwait` and sets the
/// returned flag. No handler runs in signal context and no `unsafe` is used, so
/// the shutdown path is ordinary code that can flush the sinks.
fn install_signal_handling() -> Result<Arc<AtomicBool>, String> {
    use nix::sys::signal::{SigSet, Signal};

    let mut set = SigSet::empty();
    set.add(Signal::SIGTERM);
    set.add(Signal::SIGINT);
    set.thread_block()
        .map_err(|e| format!("cannot block shutdown signals: {e}"))?;

    let flag = Arc::new(AtomicBool::new(false));
    let thread_flag = Arc::clone(&flag);
    std::thread::Builder::new()
        .name("aquila-signals".to_string())
        .spawn(move || {
            if let Ok(signal) = set.wait() {
                eprintln!("aquila-addon: received {signal:?}, shutting down");
                thread_flag.store(true, Ordering::SeqCst);
            }
        })
        .map_err(|e| format!("cannot spawn signal thread: {e}"))?;
    Ok(flag)
}

/// The distinct rooms a scene's nodes occupy, in declaration order.
fn scene_rooms(script: &SceneScript) -> Vec<RoomId> {
    let mut rooms: Vec<RoomId> = Vec::new();
    for node in &script.nodes {
        if !rooms.contains(&node.room) {
            rooms.push(node.room.clone());
        }
    }
    rooms
}

/// A deterministic two-room scene with one walking, breathing actor in the
/// lounge for the middle of each cycle, so the default run exercises presence,
/// count and vitals end to end without hardware.
fn demo_scene() -> SceneScript {
    let kitchen: RoomId = Slug("kitchen".to_string());
    let lounge: RoomId = Slug("lounge".to_string());
    SceneScript {
        name: "addon-demo".to_string(),
        home: Slug("addon-home".to_string()),
        nodes: vec![
            NodeSpec {
                id: NodeId(1),
                room: kitchen,
                pos_m: [0.0, 0.0, 2.4],
                skew_ppm: 12.0,
                noise_scale: 1.0,
            },
            NodeSpec {
                id: NodeId(2),
                room: lounge.clone(),
                pos_m: [0.0, 0.0, 2.4],
                skew_ppm: -9.0,
                noise_scale: 1.0,
            },
        ],
        actors: vec![Actor {
            room: lounge,
            position_m: [1.5, 0.0, 0.0],
            active_from_s: 2.0,
            active_to_s: 6.0,
            motion: Motion::Walk {
                axis: [1.0, 0.0, 0.0],
                amplitude_m: 0.5,
                period_s: 2.0,
                phase_rad: 0.0,
            },
            breathing: Some(Rhythm {
                amplitude_m: 0.005,
                rate_hz: 0.25,
                phase_rad: 0.0,
            }),
            heart: None,
            body: None,
        }],
        band: Band::B2G4,
        channel: 6,
        bandwidth_mhz: 20,
        subcarriers: 64,
        subcarrier_spacing_hz: 312_500.0,
        tick_hz: 20.0,
        duration_s: 8.0,
        noise_amplitude: 0.05,
        impairments: Impairments::default(),
    }
}

/// The empty-home baseline for calibration: the same nodes, no actors.
fn empty_scene(script: &SceneScript) -> SceneScript {
    let mut scene = script.clone();
    scene.name = format!("{}-empty", script.name);
    scene.actors.clear();
    scene
}

/// A labelled walk-through for calibration: one actor moving in `room` for the
/// whole capture, with the same nodes as the runtime scene.
fn walk_scene(script: &SceneScript, room: &RoomId) -> SceneScript {
    let mut scene = script.clone();
    scene.name = format!("{}-walk-{}", script.name, room.as_str());
    scene.actors = vec![Actor {
        room: room.clone(),
        position_m: [1.5, 0.0, 0.0],
        active_from_s: 0.0,
        active_to_s: scene.duration_s,
        motion: Motion::Walk {
            axis: [1.0, 0.0, 0.0],
            amplitude_m: 0.5,
            period_s: 2.0,
            phase_rad: 0.0,
        },
        breathing: Some(Rhythm {
            amplitude_m: 0.005,
            rate_hz: 0.25,
            phase_rad: 0.0,
        }),
        heart: None,
        body: None,
    }];
    scene
}

/// Build a contract topology for a scene: one floor, one room per node room.
fn topology_for_scene(script: &SceneScript) -> Topology {
    let home = script.home.clone();
    let floor: Slug = Slug("ground".to_string());
    let firmware = FirmwareRef {
        version: "addon".to_string(),
        sha256: None,
    };
    let rooms = scene_rooms(script);
    let model = ContractTopology {
        home: Home {
            id: home.clone(),
            name: script.name.clone(),
            time_zone: "Australia/Sydney".to_string(),
            contract: CONTRACT_VERSION,
        },
        floors: vec![Floor {
            id: floor.clone(),
            home: home.clone(),
            name: "Ground".to_string(),
            level: 0,
            elevation_m: 0.0,
        }],
        rooms: rooms
            .iter()
            .map(|room| Room {
                id: room.clone(),
                floor: floor.clone(),
                name: room.as_str().to_string(),
                kind: RoomKind::Unknown,
                origin_home_m: [0.0, 0.0, 0.0],
                yaw_deg: 0.0,
                area_m2: None,
            })
            .collect(),
        nodes: script
            .nodes
            .iter()
            .map(|spec| Node {
                id: spec.id,
                room: spec.room.clone(),
                name: format!("node-{}", spec.id.0),
                mac: MacAddr([0, 0, 0, 0, 0, spec.id.0 as u8]),
                device: DeviceType::Esp32S3,
                firmware: firmware.clone(),
                pos_room_m: Some([
                    spec.pos_m[0] as f32,
                    spec.pos_m[1] as f32,
                    spec.pos_m[2] as f32,
                ]),
                installed_unix_ms: 0,
            })
            .collect(),
        pairs: vec![],
    };
    // The construction above mirrors the contract's referential-integrity
    // rules, so loading cannot fail; surface it honestly if it ever does.
    Topology::load(model).expect("the composition topology is referentially valid")
}

/// Print usage.
fn print_help() {
    println!("aquila-addon — Aquila composition root\n");
    println!("USAGE:\n    aquila-addon [--host <ADDR>] [--port <PORT>]\n");
    println!("OPTIONS:");
    println!("    --host <ADDR>   bind address (default {DEFAULT_HOST}; env AQUILA_BIND_ADDR)");
    println!("    --port <PORT>   control-plane port (default {DEFAULT_PORT}; env AQUILA_PORT)");
    println!("    -h, --help      print this message\n");
    println!("ENVIRONMENT:");
    println!("    AQUILA_COMMIT         required 40-hex build commit (required)");
    println!("    {DASHBOARD_DIR_ENV}  baked dashboard bundle to serve");
    println!(
        "    {LIVE_ADDR_ENV}       live CSI UDP bind address (default: repeat the demo scene)"
    );
    println!(
        "\nWith no options, binds {DEFAULT_HOST}:{DEFAULT_PORT}, serves the dashboard and the"
    );
    println!("/api/v1 API, calibrates from a synthetic empty home, and repeats the demo scene.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use aquila_metrics::InMemoryMetricSink;
    use aquila_sensing::{Sink, SinkError};
    use aquila_types::Event;
    use std::sync::Mutex;

    const TEST_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    /// A `Sink` that counts the events and measurements the core emits.
    #[derive(Default)]
    struct CountingSink {
        events: Arc<Mutex<usize>>,
    }

    impl Sink for CountingSink {
        fn emit(&mut self, _event: &Event) -> Result<(), SinkError> {
            *self.events.lock().expect("lock") += 1;
            Ok(())
        }
    }

    #[test]
    fn the_demo_scene_calibrates_and_emits_presence() {
        let script = demo_scene();
        let topology = topology_for_scene(&script);
        let inputs = calibration_inputs(&script).expect("calibration inputs");

        let frames = collect_frames(&script).expect("frames");
        assert!(!frames.is_empty(), "the demo scene must synthesise frames");
        let source = RepeatingSource::new(frames, script.duration_s);

        let events = Arc::new(Mutex::new(0usize));
        let sink = CountingSink {
            events: Arc::clone(&events),
        };
        let clock: Arc<dyn Clock> = Arc::new(SystemClock::new());
        let config = SensingConfig::default().with_commit(TEST_COMMIT);
        let mut sensing = Sensing::new(
            Box::new(source),
            clock,
            Box::new(sink),
            Box::new(InMemoryMetricSink::new()),
            None,
            None,
            topology.model().clone(),
            config,
        )
        .expect("valid core");

        let request = CalibrationRequest::new(TEST_COMMIT, "addon-test", CALIBRATION_GENERATED_AT);
        let report = sensing.calibrate(&request, &inputs).expect("calibration");
        assert!(report.calibrated, "calibration note: {}", report.note);
        assert!(report.artifact.is_some());

        // Two passes cover the actor's 2..6 s presence span and its absence.
        let mut emitted = 0usize;
        for _ in 0..700 {
            let step = sensing.step().expect("step");
            assert!(!step.ended, "the repeating source must never end");
            emitted += step.events_emitted;
        }
        assert!(
            emitted > 0 && *events.lock().expect("lock") == emitted,
            "the calibrated demo must emit events through the sink (emitted {emitted})"
        );
    }
}
