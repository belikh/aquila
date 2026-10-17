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
//!   flush the sinks, stop the listener, exit zero; and
//! - resolves a stable `instance_id` (env → persisted `<data>/instance_id` →
//!   generated once and persisted) so the HACS component's devices and entities
//!   keep their identity across restarts (`home-assistant.md` §4, §9).
//!
//! A production build swaps the simulator for `UdpLiveSource` and supplies real
//! models; the wiring shape is identical.
//!
//! # Prose
//!
//! Australian English throughout (serialisation, behaviour, normalise).

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::time::{Duration, Instant, UNIX_EPOCH};

use aquila_api::{Auth, ChannelSink, FirmwareStore, Ota, OtaError, OtaTransport, Server, ServerConfig, StaticAssets};
use aquila_geometry::Topology;
use aquila_metrics::InMemoryMetricSink;
use aquila_sensing::{CalibrationInputs, CalibrationRequest, Sensing, SensingConfig, SystemClock};
use aquila_simulator::{
    Actor, Impairments, Motion, NodeSpec, Rhythm, SceneScript, SimConfig, SimulatorSource,
};
use aquila_sources::{CsiSource, FrameOrEnd, SourceError};
use aquila_types::{
    Band, Clock, CsiEnvelope, DeviceType, FirmwareRef, Home, MacAddr, Node, NodeId, Slug,
    Topology as ContractTopology, Zone, ZoneId, CONTRACT_VERSION,
};
use aquila_wire::{
    ControlBody, ControlMessage, decode_control, decode_control_authenticated,
    decode_datagram, encode_control, encode_control_authenticated, JoinAck, JoinRequest,
    MAX_DATAGRAM_LEN, CONTROL_AUTH_LEN, CONTROL_PORT, result_message,
};
use ed25519_dalek::VerifyingKey;

/// The default control-plane port (`core-api.md` §2).
const DEFAULT_PORT: u16 = 8099;

/// The default bind address: every interface, so the container is reachable
/// through Home Assistant ingress.
const DEFAULT_HOST: IpAddr = IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED);

/// The environment variable naming the baked dashboard bundle directory.
const DASHBOARD_DIR_ENV: &str = aquila_api::DASHBOARD_DIR_ENV;

/// When set, names the core instance directly and takes precedence over the
/// persisted file (`home-assistant.md` §9).
const INSTANCE_ID_ENV: &str = "AQUILA_INSTANCE_ID";

/// The directory the core persists state in. The Home Assistant add-on mounts
/// `/data`; a local run can point this elsewhere (`home-assistant.md` §9).
const DATA_DIR_ENV: &str = "AQUILA_DATA_DIR";

/// The Home Assistant add-on's persistent data directory.
const DEFAULT_DATA_DIR: &str = "/data";

/// The file name the generated instance id is persisted in.
const INSTANCE_ID_FILE: &str = "instance_id";

/// When set, the composition root pulls live CSI from this UDP address instead
/// of repeating the deterministic simulator scene.
const LIVE_ADDR_ENV: &str = "AQUILA_LIVE_ADDR";

/// The deterministic RNG seed for the demo scene and its calibration captures.
const DEMO_SEED: u64 = 2026;

/// A fixed, deterministic generation stamp for the startup calibration. The
/// wizard reads no wall clock, so the artefact is reproducible run to run.
const CALIBRATION_GENERATED_AT: &str = "2026-01-01T00:00:00Z";

/// Default CSI data-plane bind address: every interface, so real nodes on the
/// LAN can reach the add-on. Overridden by `AQUILA_LIVE_ADDR` when set.
const DEFAULT_LIVE_ADDR: &str = "0.0.0.0:9100";

/// The environment variable naming the signed firmware image store directory.
const FIRMWARE_DIR_ENV: &str = "AQUILA_FIRMWARE_DIR";

/// The default firmware store directory under the add-on's persistent data dir.
const DEFAULT_FIRMWARE_DIR: &str = "/data/aquila/firmware";

/// The environment variable naming the compiled-in Ed25519 firmware trust
/// anchor as 64 hex characters. `AQUILA_FW_PUBKEY_HEX`.
const FW_PUBKEY_ENV: &str = "AQUILA_FW_PUBKEY_HEX";

/// The topology document persisted under the add-on's data directory.
const TOPOLOGY_FILE_NAME: &str = "topology.json";

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
    // P4: load the operator topology from disk, falling back to the demo scene
    // topology when nothing has been provisioned yet. Real nodes are mapped to
    // rooms from this topology.
    let data_dir = PathBuf::from(
        std::env::var(DATA_DIR_ENV).unwrap_or_else(|_| DEFAULT_DATA_DIR.to_string()),
    );
    let topology = load_topology(&script, &data_dir)?;
    let mut sensing_config = SensingConfig::default().with_commit(commit.clone());
    // The add-on surfaces vitals to Home Assistant (`home-assistant.md` §6.1), so
    // the core estimates breathing and heart rate alongside presence and count.
    sensing_config.vitals_enabled = true;

    // Block SIGTERM/SIGINT before any worker thread exists, so every thread
    // inherits the mask and only the dedicated `sigwait` thread observes them.
    let shutdown = install_signal_handling()?;

    let addr = SocketAddr::new(options.host, options.port);
    // A stable instance id is what the HACS component keys its devices and
    // entities to (`home-assistant.md` §4, §9), so resolve it once and persist
    // the generated value under `/data`.
    let (instance_id, instance_source) = resolve_instance_id();
    let assets = StaticAssets::from_env();
    let server = Server::bind_with_config(
        addr,
        &topology,
        instance_id.clone(),
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

    // ------------------------------------------------------------------
    // P2: node control channel (pairing, config, heartbeats) on UDP 9101.
    // ------------------------------------------------------------------
    let control_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), CONTROL_PORT);
    let control = Arc::new(
        ControlServer::bind(control_addr)
            .map_err(|e| format!("cannot bind node control channel on {control_addr}: {e}"))?,
    );
    let pairing_codes = read_pairing_codes();
    let auto_code = if pairing_codes.is_empty() {
        Some(random_u32())
    } else {
        None
    };
    {
        let mut reg = control.registry.lock().expect("registry poisoned");
        for code in &pairing_codes {
            reg.add_pairing_code(*code);
        }
        if let Some(code) = auto_code {
            reg.add_pairing_code(code);
            println!(
                "aquila-addon: pairing code {code} (enter it on each node; set AQUILA_PAIRING_CODES to pre-configure)"
            );
        }
    }
    let control_runner = Arc::clone(&control);
    let ota_runner = handle.ota();
    let registry_runner = Arc::clone(&control.registry);
    let model_runner = topology.model().clone();
    let data_dir_runner = data_dir.clone();
    let shutdown_runner = Arc::clone(&shutdown);
    std::thread::Builder::new()
        .name("aquila-control".to_string())
        .spawn(move || {
            if let Err(e) = ControlServer::run(
                control_runner,
                ota_runner,
                registry_runner,
                model_runner,
                data_dir_runner,
                shutdown_runner,
            ) {
                eprintln!("aquila-addon: control server stopped: {e}");
            }
        })
        .map_err(|e| format!("cannot spawn control server: {e}"))?;

    // ------------------------------------------------------------------
    // P3: signed firmware image store. Without a trust anchor OTA stays 503.
    // ------------------------------------------------------------------
    if let Some(hex) = std::env::var(FW_PUBKEY_ENV).ok().filter(|s| !s.is_empty()) {
        let key = parse_fw_pubkey(&hex).map_err(|e| format!("AQUILA_FW_PUBKEY_HEX: {e}"))?;
        let fw_dir = std::env::var(FIRMWARE_DIR_ENV).unwrap_or_else(|_| DEFAULT_FIRMWARE_DIR.to_string());
        let store = FirmwareStore::open_default(key)
            .map_err(|e| format!("firmware store at {fw_dir}: {e}"))?;
        handle.ota().configure_store(store);
        println!("aquila-addon: firmware OTA store at {fw_dir}; images can be imported via the dashboard");
    } else {
        eprintln!("aquila-addon: AQUILA_FW_PUBKEY_HEX not set; firmware OTA is unavailable until a trust anchor is provisioned");
    }

    println!("aquila-addon: control plane on http://{bound}/api/v1/health");
    println!("aquila-addon: health  http://{bound}/api/v1/health");
    println!("aquila-addon: instance_id {instance_id} (source: {instance_source})");
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

/// Resolve the core instance id, deterministically and stably.
///
/// Precedence (`home-assistant.md` §9, §4):
/// 1. `AQUILA_INSTANCE_ID`, when set and non-empty;
/// 2. `<AQUILA_DATA_DIR or /data>/instance_id`, when present and non-empty;
/// 3. a freshly generated RFC 4122 version-4 UUID, persisted to that file so
///    the same id is used on the next start.
///
/// The returned string is the source label (`env`, `file`, `generated` or
/// `in-memory`) for the startup banner. If the data directory is not writable
/// the generated id is used in memory for this run and a warning is logged, so
/// the core still starts.
fn resolve_instance_id() -> (String, &'static str) {
    if let Ok(value) = std::env::var(INSTANCE_ID_ENV) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return (trimmed.to_string(), "env");
        }
    }

    let data_dir = std::env::var(DATA_DIR_ENV).unwrap_or_else(|_| DEFAULT_DATA_DIR.to_string());
    let path = std::path::Path::new(&data_dir).join(INSTANCE_ID_FILE);
    if let Some(existing) = read_instance_id(&path) {
        return (existing, "file");
    }

    let generated = random_uuid_v4();
    match persist_instance_id(&path, &generated) {
        Ok(()) => (generated, "generated"),
        Err(error) => {
            eprintln!(
                "aquila-addon: cannot persist instance id to {}: {error}; \
                 using an in-memory id for this run",
                path.display()
            );
            (generated, "in-memory")
        }
    }
}

/// Read a non-empty instance id from `path`, if the file exists.
fn read_instance_id(path: &std::path::Path) -> Option<String> {
    let value = std::fs::read_to_string(path).ok()?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Write `id` to `path`, creating the parent directory if needed.
fn persist_instance_id(path: &std::path::Path, id: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("{id}\n"))
}

/// Generate an RFC 4122 version-4 UUID without a new dependency or `unsafe`.
///
/// Sixteen bytes are read from `/dev/urandom`; the version and variant bits are
/// then set. If `/dev/urandom` is unavailable (a non-Unix or restricted
/// sandbox), a time- and pid-seeded value is used so the core still starts with
/// a unique-enough id rather than failing.
fn random_uuid_v4() -> String {
    let mut bytes = [0u8; 16];
    if read_random(&mut bytes).is_err() {
        seed_bytes(&mut bytes);
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // Version 4.
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // Variant 10xx.
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32],
    )
}

/// Fill `bytes` from `/dev/urandom`.
fn read_random(bytes: &mut [u8]) -> std::io::Result<()> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")?.read_exact(bytes)
}

/// Fill `bytes` from a time- and pid-seeded xorshift, for the rare host with no
/// `/dev/urandom`.
fn seed_bytes(bytes: &mut [u8]) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos() as u64);
    let mut state = nanos ^ (u64::from(std::process::id()) << 32) ^ 0x9e37_79b9_7f4a_7c15;
    for byte in bytes.iter_mut() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *byte = (state & 0xff) as u8;
    }
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
    for zone in scene_zones(script) {
        let frames = collect_frames(&walk_scene(script, &zone))?;
        walkthrough.push((zone, frames));
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

/// Build the CSI source for the run. By default the add-on binds the CSI data
/// plane (`0.0.0.0:9100`, overridable with `AQUILA_LIVE_ADDR`) so real nodes
/// can stream raw CSI straight into the sensing core. The deterministic demo
/// scene is only a fallback when the live socket cannot be bound — it is not
/// the production path.
fn build_source(
    script: &SceneScript,
    shutdown: &Arc<AtomicBool>,
    clock: Arc<dyn Clock>,
) -> Result<(Box<dyn CsiSource>, Option<Duration>), String> {
    let addr = std::env::var(LIVE_ADDR_ENV).unwrap_or_else(|_| DEFAULT_LIVE_ADDR.to_string());
    let addr: SocketAddr = addr
        .parse()
        .map_err(|e| format!("invalid {LIVE_ADDR_ENV} `{addr}`: {e}"))?;
    match UdpSocket::bind(addr) {
        Ok(socket) => {
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
        Err(error) => {
            eprintln!(
                "aquila-addon: cannot bind live CSI socket on {addr}: {error}; \
                 falling back to the repeating demo scene"
            );
        }
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

/// The distinct zones a scene's nodes occupy, in declaration order.
fn scene_zones(script: &SceneScript) -> Vec<ZoneId> {
    let mut zones: Vec<ZoneId> = Vec::new();
    for node in &script.nodes {
        if !zones.contains(&node.zone) {
            zones.push(node.zone.clone());
        }
    }
    zones
}

/// A deterministic two-room scene with one walking, breathing actor in the
/// lounge for the middle of each cycle, so the default run exercises presence,
/// count and vitals end to end without hardware.
fn demo_scene() -> SceneScript {
    let lounge: ZoneId = Slug("lounge".to_string());
    SceneScript {
        name: "addon-demo".to_string(),
        home: Slug("addon-home".to_string()),
        nodes: vec![
            NodeSpec {
                id: NodeId(1),
                zone: lounge.clone(),
                pos_m: [0.0, 0.0, 2.4],
                skew_ppm: 12.0,
                noise_scale: 1.0,
            },
            NodeSpec {
                id: NodeId(2),
                zone: lounge.clone(),
                pos_m: [3.0, 0.0, 2.4],
                skew_ppm: -9.0,
                noise_scale: 1.0,
            },
        ],
        actors: vec![Actor {
            zone: lounge.clone(),
            position_m: [1.5, 0.0, 0.0],
            active_from_s: 2.0,
            active_to_s: 6.0,
            motion: Motion::Walk {
                axis: [1.0, 0.0, 0.0],
                amplitude_m: 2.0,
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

/// A labelled walk-through for calibration: one actor moving in `zone` for the
/// whole capture, with the same nodes as the runtime scene.
fn walk_scene(script: &SceneScript, zone: &ZoneId) -> SceneScript {
    let mut scene = script.clone();
    scene.name = format!("{}-walk-{}", script.name, zone.as_str());
    // Place the actor at the zone's origin (first node in that zone) so the
    // walk-through capture has an actor inside the zone.
    let pos = script.nodes.iter()
        .find(|n| n.zone == *zone)
        .map(|n| n.pos_m)
        .unwrap_or([1.5, 0.0, 0.0]);
    scene.actors = vec![Actor {
        zone: zone.clone(),
        position_m: pos,
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

/// Build a contract topology for a scene: one zone per node zone.
fn topology_for_scene(script: &SceneScript) -> Topology {
    let home = script.home.clone();
    let firmware = FirmwareRef {
        version: "addon".to_string(),
        sha256: None,
    };
    let zones = scene_zones(script);
    let model = ContractTopology {
        home: Home {
            id: home.clone(),
            name: script.name.clone(),
            time_zone: "Australia/Sydney".to_string(),
            contract: CONTRACT_VERSION,
        },
        zones: zones
            .iter()
            .map(|zone| {
                // Distinct zone origins per node zone so zones do not overlap and
                // calibration can attribute each zone separately.
                let origin = script
                    .nodes
                    .iter()
                    .find(|n| n.zone == *zone)
                    .map(|n| n.pos_m)
                    .unwrap_or([0.0, 0.0, 0.0]);
                // Zone origins spaced 5m apart so calibration can separate them.
                let zone_index = zones.iter().position(|z| z == zone).unwrap_or(0);
                Zone {
                    id: zone.clone(),
                    origin_home_m: [
                        origin[0] as f32 + zone_index as f32 * 5.0,
                        origin[1] as f32,
                        origin[2] as f32,
                    ],
                    extent_home_m: [2.0, 2.0, 2.0],
                    name: zone.as_str().to_string(),
                }
            })
            .collect(),
        nodes: script
            .nodes
            .iter()
            .map(|spec| Node {
                id: spec.id,
                zone: spec.zone.clone(),
                name: format!("node-{}", spec.id.0),
                mac: MacAddr([0, 0, 0, 0, 0, spec.id.0 as u8]),
                device: DeviceType::Esp32S3,
                firmware: firmware.clone(),
                pos_home_m: [
                    spec.pos_m[0] as f32,
                    spec.pos_m[1] as f32,
                    spec.pos_m[2] as f32,
                ],
                installed_unix_ms: 0,
            })
            .collect(),
        pairs: vec![],
    };
    // The construction above mirrors the contract's referential-integrity
    // rules, so loading cannot fail; surface it honestly if it ever does.
    Topology::load(model).expect("the composition topology is referentially valid")
}

/// The subdirectory under the add-on data directory that holds the persisted
/// operator topology document (`home-assistant.md` §9).
const TOPOLOGY_DIR_NAME: &str = "aquila";

/// The filesystem path of the persisted topology document.
fn topology_path(data_dir: &Path) -> PathBuf {
    data_dir.join(TOPOLOGY_DIR_NAME).join(TOPOLOGY_FILE_NAME)
}

/// Load the operator topology from disk, falling back to the demo scene
/// topology when nothing has been provisioned yet (`P4`).
fn load_topology(script: &SceneScript, data_dir: &Path) -> Result<Topology, String> {
    let path = topology_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let model: aquila_types::Topology = serde_json::from_str(&text)
                .map_err(|e| format!("topology at {} is not valid JSON: {e}", path.display()))?;
            Topology::load(model).map_err(|e| {
                format!("topology at {} failed referential integrity: {e}", path.display())
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("aquila-addon: no topology at {}; using the demo scene topology", path.display());
            Ok(topology_for_scene(script))
        }
        Err(e) => Err(format!("cannot read topology at {}: {e}", path.display())),
    }
}

// -----------------------------------------------------------------------------
// P2/P3: node control channel (UDP 9101) + OTA transport wiring.

/// Valid (short-lived) pairing codes. A code is consumed the first time a node
/// presents it in `JOIN_REQUEST`.
#[derive(Default)]
struct NodeRegistry {
    pairings: HashMap<u32, ()>,
    /// Paired nodes, keyed by factory MAC.
    paired: HashMap<MacAddr, PairedNode>,
    /// Socket address -> factory MAC, so incoming datagrams are attributed.
    addr_to_mac: HashMap<SocketAddr, MacAddr>,
    /// Per-node OTA reply channel (node -> core direction), fed by the control
    /// receive loop and drained by the node's `OtaTransport`.
    ota_tx: HashMap<MacAddr, mpsc::Sender<ControlMessage>>,
}

impl NodeRegistry {
    fn new() -> Self {
        Self::default()
    }

    fn add_pairing_code(&mut self, code: u32) {
        self.pairings.insert(code, ());
    }

    /// Dense node id for a factory MAC, from the provisioned topology.
    fn node_for_mac(&self, mac: &MacAddr, model: &aquila_types::Topology) -> Option<NodeId> {
        model.nodes.iter().find(|n| &n.mac == mac).map(|n| n.id)
    }
}

/// A node that has completed pairing.
#[derive(Clone)]
struct PairedNode {
    /// Factory MAC address.
    mac: MacAddr,
    /// Dense node id assigned by the core (from the topology, or allocated).
    node_id: NodeId,
    /// Long-lived pairing token; authenticates every subsequent core->node
    /// command (`capture-and-firmware.md` §6.3).
    token: [u8; CONTROL_AUTH_LEN],
    /// The socket address the node's datagrams arrive from.
    addr: SocketAddr,
}

/// The node control plane: one UDP socket on `CONTROL_PORT` (9101), serving
/// `AQCC` request/response (`capture-and-firmware.md` §6).
struct ControlServer {
    socket: UdpSocket,
    registry: Arc<Mutex<NodeRegistry>>,
}

impl ControlServer {
    fn bind(addr: SocketAddr) -> Result<Self, SourceError> {
        let socket = UdpSocket::bind(addr).map_err(SourceError::Io)?;
        socket
            .set_read_timeout(Some(Duration::from_millis(200)))
            .map_err(SourceError::Io)?;
        Ok(Self {
            socket,
            registry: Arc::new(Mutex::new(NodeRegistry::new())),
        })
    }

    /// Send one control message to a paired node, authenticated with its token.
    fn send_to(&self, mac: &MacAddr, message: &ControlMessage) -> Result<(), SourceError> {
        let registry = self.registry.lock().expect("registry poisoned");
        let node = registry.paired.get(mac).ok_or_else(|| {
            SourceError::Other(format!("node {} is not paired", mac_hex(mac)))
        })?;
        let bytes = encode_control_authenticated(message, &node.token)
            .map_err(|e| SourceError::Decode(e))?;
        self.socket.send_to(&bytes, node.addr).map_err(SourceError::Io)?;
        Ok(())
    }

    /// Receive and dispatch control datagrams until `shutdown` is set.
    #[allow(clippy::too_many_arguments)]
    fn run(
        server: Arc<Self>,
        ota: Arc<Ota>,
        registry: Arc<Mutex<NodeRegistry>>,
        model: aquila_types::Topology,
        data_dir: PathBuf,
        shutdown: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let mut buf = vec![0u8; MAX_DATAGRAM_LEN];
        while !shutdown.load(Ordering::Relaxed) {
            match server.socket.recv_from(&mut buf) {
                Ok((len, from)) => {
                    let bytes = &buf[..len];
                    // Attributed sender: paired nodes are spoken to with their
                    // token; everything else is plain framing.
                    let mac = {
                        let reg = registry.lock().expect("registry");
                        reg.addr_to_mac.get(&from).copied()
                    };
                    let message = match mac {
                        Some(mac) => {
                            let token = {
                                let reg = registry.lock().expect("registry");
                                reg.paired.get(&mac).map(|n| n.token).unwrap_or([0u8; CONTROL_AUTH_LEN])
                            };
                            match decode_control_authenticated(bytes, &token) {
                                Ok(m) => m,
                                Err(_) => continue, // tampered or unauthenticated from a paired node
                            }
                        }
                        None => match decode_control(bytes) {
                            Ok(m) => m,
                            Err(_) => continue,
                        },
                    };
                    if let Some(transport) =
                        Self::dispatch(&server, message, from, &registry, &model, &data_dir)
                    {
                        ota.register_transport(format!("{:?}", transport.mac()), Box::new(transport));
                    }
                }
                Err(e) if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) => continue,
                Err(e) => return Err(format!("control socket error: {e}")),
            }
        }
        Ok(())
    }

    /// Dispatch one decoded control message. Returns an OTA transport to
    /// register when a node just paired.
    #[allow(clippy::too_many_arguments)]
    fn dispatch(
        server: &Arc<Self>,
        message: ControlMessage,
        from: SocketAddr,
        registry: &Arc<Mutex<NodeRegistry>>,
        model: &aquila_types::Topology,
        data_dir: &Path,
    ) -> Option<UdpOtaTransport> {
        match message.body {
            ControlBody::JoinRequest(req) => {
                Self::handle_join(server, message.request_id, req, from, registry, model, data_dir)
            }
            ControlBody::Status(_status) => {
                // Heartbeat from a paired node. The online count is surfaced by
                // the OTA agent health (#14); here we log it.
                let reg = registry.lock().expect("registry");
                if let Some(node) = reg.paired.values().find(|n| n.addr == from).cloned() {
                    drop(reg);
                    eprintln!("aquila-addon: STATUS from node {} ({})", node.node_id.0, mac_hex(&node.mac));
                }
                None
            }
            ControlBody::OtaAck(_)
            | ControlBody::OtaStatus(_)
            | ControlBody::OtaResult(_)
            | ControlBody::OtaAbort(_) => {
                Self::route_ota_reply(server, message, from, registry);
                None
            }
            _ => {
                // Acknowledge any other command with RESULT Active.
                let reg = registry.lock().expect("registry");
                let mac = reg.addr_to_mac.get(&from).copied();
                drop(reg);
                if let Some(mac) = mac {
                    let _ = server.send_result(from, mac, message.request_id, aquila_wire::AckStatus::Active);
                }
                None
            }
        }
    }

    /// A node is attempting to pair: validate its code, issue a token, and
    /// return its OTA transport for registration.
    #[allow(clippy::too_many_arguments)]
    fn handle_join(
        server: &Arc<Self>,
        request_id: u32,
        req: JoinRequest,
        from: SocketAddr,
        registry: &Arc<Mutex<NodeRegistry>>,
        model: &aquila_types::Topology,
        data_dir: &Path,
    ) -> Option<UdpOtaTransport> {
        let mut reg = registry.lock().expect("registry");
        if !reg.pairings.contains_key(&req.pairing_code) {
            drop(reg);
            let _ = server.send_result_unauth(from, request_id, aquila_wire::AckStatus::Rejected, "invalid pairing code");
            return None;
        }
        reg.pairings.remove(&req.pairing_code);

        let token = random_token();
        let node_id = reg.node_for_mac(&req.node, model).unwrap_or_else(|| {
            let max = reg.paired.values().map(|n| n.node_id.0).max().unwrap_or(0);
            NodeId(max + 1)
        });

        let (tx, rx) = mpsc::channel();
        reg.ota_tx.insert(req.node, tx);
        reg.addr_to_mac.insert(from, req.node);
        reg.paired.insert(
            req.node,
            PairedNode { mac: req.node, node_id, token, addr: from },
        );
        drop(reg);

        let transport = UdpOtaTransport {
            server: Arc::clone(server),
            mac: req.node,
            rx,
        };

        // JOIN_ACK is intentionally unauthenticated: it delivers the token
        // (`capture-and-firmware.md` §6.3).
        let ack = JoinAck {
            node: req.node,
            token,
            config_version: 0,
            config_hash: [0u8; CONTROL_AUTH_LEN],
        };
        let msg = ControlMessage { request_id, body: ControlBody::JoinAck(ack) };
        let bytes = encode_control(&msg).expect("encode JOIN_ACK");
        let _ = server.socket.send_to(&bytes, from);

        println!(
            "aquila-addon: node {} (factory {}) paired as node {} from {}",
            node_id.0,
            mac_hex(&req.node),
            node_id.0,
            from,
        );
        let _ = data_dir; // topology save-on-pair is a future enhancement
        Some(transport)
    }

    /// Route a node->core OTA reply (ACK/STATUS/RESULT/ABORT) into the
    /// originating node's transport channel.
    fn route_ota_reply(
        server: &Arc<Self>,
        message: ControlMessage,
        from: SocketAddr,
        registry: &Arc<Mutex<NodeRegistry>>,
    ) {
        let _ = server;
        let reg = registry.lock().expect("registry");
        let mac = reg.addr_to_mac.get(&from).copied();
        let tx = mac.and_then(|m| reg.ota_tx.get(&m).cloned());
        drop(reg);
        if let Some(tx) = tx {
            let _ = tx.send(message);
        }
    }

    /// Send a `RESULT` reply, authenticated with the paired node's token.
    fn send_result(
        &self,
        _from: SocketAddr,
        mac: MacAddr,
        request_id: u32,
        status: aquila_wire::AckStatus,
    ) -> Result<(), SourceError> {
        let reg = self.registry.lock().expect("registry");
        let node = reg.paired.get(&mac).ok_or_else(|| {
            SourceError::Other(format!("node {} is not paired", mac_hex(&mac)))
        })?;
        let msg = result_message(request_id, status, "");
        let bytes = encode_control_authenticated(&msg, &node.token)
            .map_err(|e| SourceError::Decode(e))?;
        self.socket.send_to(&bytes, node.addr).map_err(SourceError::Io)?;
        Ok(())
    }

    /// Send a `RESULT` reply without authentication (pre-pairing, e.g. join
    /// rejection).
    fn send_result_unauth(
        &self,
        to: SocketAddr,
        request_id: u32,
        status: aquila_wire::AckStatus,
        detail: &str,
    ) -> Result<(), SourceError> {
        let msg = result_message(request_id, status, detail);
        let bytes = encode_control(&msg).map_err(|e| SourceError::Decode(e))?;
        self.socket.send_to(&bytes, to).map_err(SourceError::Io)?;
        Ok(())
    }
}

/// A node's core->node OTA transport: sends over the control channel and drains
/// the node's OTA reply channel. Implements the [`OtaTransport`] seam so the
/// manager's production path and a fake are the same code.
struct UdpOtaTransport {
    server: Arc<ControlServer>,
    mac: MacAddr,
    rx: mpsc::Receiver<ControlMessage>,
}

impl UdpOtaTransport {
    #[must_use]
    fn mac(&self) -> MacAddr {
        self.mac
    }
}

impl OtaTransport for UdpOtaTransport {
    fn send(&mut self, message: &ControlMessage) -> Result<(), OtaError> {
        self.server.send_to(&self.mac, message).map_err(|e| OtaError::Transport(e.to_string()))
    }

    fn try_recv(&mut self) -> Result<Option<ControlMessage>, OtaError> {
        Ok(self.rx.try_recv().ok())
    }
}

/// Generate a 32-byte pairing token without the `rand` crate.
fn random_token() -> [u8; CONTROL_AUTH_LEN] {
    let mut bytes = [0u8; CONTROL_AUTH_LEN];
    let mut state = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
        ^ (u64::from(std::process::id()) << 32);
    for b in bytes.iter_mut() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *b = (state & 0xff) as u8;
    }
    bytes
}

/// Render a MAC as colon-separated hex for logs.
fn mac_hex(mac: &MacAddr) -> String {
    mac.0.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":")
}

/// Read valid pairing codes from `AQUILA_PAIRING_CODES` (comma-separated u32).
/// When none are configured, one random code is generated and printed so the
/// operator can enter it on the node (`capture-and-firmware.md` §6.3).
fn read_pairing_codes() -> Vec<u32> {
    match std::env::var("AQUILA_PAIRING_CODES") {
        Ok(value) => {
            let mut codes = Vec::new();
            for part in value.split(',') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                if let Ok(code) = part.parse::<u32>() {
                    codes.push(code);
                }
            }
            codes
        }
        Err(_) => Vec::new(),
    }
}

/// A pseudo-random u32 for an auto-generated pairing code.
fn random_u32() -> u32 {
    let mut state = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
        ^ (u64::from(std::process::id()) << 32);
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    state as u32
}

/// Parse a 64-hex-character Ed25519 public key (the firmware trust anchor).
fn parse_fw_pubkey(hex: &str) -> Result<VerifyingKey, String> {
    let bytes = parse_hex32(hex)?;
    VerifyingKey::from_bytes(&bytes).map_err(|e| format!("{e}"))
}

/// Decode exactly 32 bytes from 64 hex characters.
fn parse_hex32(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64 {
        return Err("expected 64 hexadecimal characters".to_string());
    }
    let bytes = hex.as_bytes();
    let mut out = [0u8; 32];
    for i in 0..32 {
        let hi = hex_digit(bytes[2 * i])?;
        let lo = hex_digit(bytes[2 * i + 1])?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

/// Decode one lowercase/uppercase hex digit.
fn hex_digit(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("invalid hex digit {}", b as char)),
    }
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
    println!("    {INSTANCE_ID_ENV}    override the resolved core instance id");
    println!("    {DATA_DIR_ENV}        persistent data directory (default {DEFAULT_DATA_DIR})");
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
        match sensing.calibrate(&request, &inputs) {
            Ok(report) => {
                assert!(report.calibrated, "calibration note: {}", report.note);
                assert!(report.artifact.is_some());
                eprintln!("CALIBRATION SUCCESS: {:#?}", report.quality);
            }
            Err(e) => {
                eprintln!("CALIBRATION FAILED: {e:?}");
                panic!("calibration failed: {e}");
            }
        }

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

    #[test]
    fn a_generated_instance_id_is_a_version_4_uuid() {
        let id = random_uuid_v4();
        assert_eq!(id.len(), 36, "{id}");
        let groups: Vec<usize> = id.split('-').map(str::len).collect();
        assert_eq!(groups, vec![8, 4, 4, 4, 12], "{id}");
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
            "{id}"
        );
        assert_eq!(&id[14..15], "4", "version nibble: {id}");
        assert!(
            matches!(&id[19..20], "8" | "9" | "a" | "b"),
            "RFC 4122 variant nibble: {id}"
        );
    }

    #[test]
    fn the_instance_id_file_round_trips() {
        let dir = std::env::temp_dir().join(format!(
            "aquila-addon-instance-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let path = dir.join(INSTANCE_ID_FILE);

        // Absent file → no id.
        assert_eq!(read_instance_id(&path), None);

        let value = "11111111-2222-4333-8444-555555555555";
        persist_instance_id(&path, value).expect("persist");
        assert_eq!(read_instance_id(&path).as_deref(), Some(value));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
