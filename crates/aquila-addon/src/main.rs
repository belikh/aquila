//! `aquila-addon` — the composition root (`architecture.md` §9).
//!
//! This binary holds **no domain logic**: it loads the topology, constructs the
//! adapters, wires them into `aquila-sensing`, and starts the private loopback
//! control plane (`aquila-api`). It is unlayered and exempt from the dependency
//! level rule.
//!
//! For a runnable demonstration it wires the deterministic `SimulatorSource`
//! into the core and publishes through the API's `ChannelSink`, so the whole
//! pipeline runs with no hardware. A production build swaps the simulator for
//! `UdpLiveSource` and supplies real models; the wiring shape is identical.

#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::sync::Arc;

use aquila_api::{ChannelSink, Server};
use aquila_geometry::Topology;
use aquila_metrics::InMemoryMetricSink;
use aquila_sensing::{Sensing, SensingConfig, SystemClock};
use aquila_simulator::{
    Actor, Impairments, Motion, NodeSpec, SceneScript, SimConfig, SimulatorSource,
};
use aquila_sources::CsiSource;
use aquila_types::{
    DeviceType, FirmwareRef, Floor, Home, MacAddr, Node, NodeId, Room, RoomId, RoomKind, Slug,
    Topology as ContractTopology, Band, CONTRACT_VERSION,
};

/// The default control-plane port (`core-api.md` §2).
const DEFAULT_PORT: u16 = 8099;

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
    port: u16,
}

/// Parse the minimal CLI: `--help` and an optional `--port`.
fn parse(args: &[String]) -> Command {
    let mut port = DEFAULT_PORT;
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
            other => return Command::Error(format!("unknown argument `{other}`")),
        }
        index += 1;
    }
    Command::Run(Options { port })
}

/// Wire the pipeline and run it.
fn run(options: &Options) -> Result<(), String> {
    let script = demo_scene();
    let config = SimConfig::new(2026, Slug("addon-demo".to_string()), 0, 0);
    let topology = topology_for_scene(&script);

    let listener: SocketAddr = format!("127.0.0.1:{}", options.port)
        .parse()
        .map_err(|e| format!("invalid bind address: {e}"))?;
    let server = Server::bind(listener, &topology, "addon-demo")
        .map_err(|e| format!("cannot bind control plane: {e}"))?;
    let address = server
        .local_addr()
        .map_err(|e| format!("cannot read local address: {e}"))?;
    let handle = server.handle();
    let mut guard = server
        .serve_in_background()
        .map_err(|e| format!("cannot serve control plane: {e}"))?;

    println!("aquila-addon: control plane on http://{address}/api/v1/health");

    let source: Box<dyn CsiSource> = Box::new(
        SimulatorSource::new(script.clone(), config.clone())
            .map_err(|e| format!("cannot build simulator: {e}"))?,
    );
    let clock = Arc::new(SystemClock::new());
    let sink = Box::new(ChannelSink::new(handle.clone()));
    let metrics = Box::new(InMemoryMetricSink::new());

    let mut sensing = Sensing::new(
        source,
        clock,
        sink,
        metrics,
        None,
        None,
        topology.model().clone(),
        SensingConfig::default(),
    )
    .map_err(|e| format!("cannot build the sensing core: {e}"))?;

    loop {
        let report = sensing.step().map_err(|e| format!("step failed: {e}"))?;
        if report.ended {
            break;
        }
    }
    sensing.sink().map_err(|e| format!("flush failed: {e}"))?;

    let revision = handle.revision();
    println!(
        "aquila-addon: run complete — {revision} events published (model revision {revision})"
    );
    guard.shutdown();
    Ok(())
}

/// A deterministic single-node scene with one walking actor, so the default run
/// exercises presence detection end to end without hardware.
fn demo_scene() -> SceneScript {
    let room: RoomId = Slug("lounge".to_string());
    SceneScript {
        name: "addon-demo".to_string(),
        home: Slug("addon-home".to_string()),
        nodes: vec![NodeSpec {
            id: NodeId(1),
            room: room.clone(),
            pos_m: [0.0, 0.0, 2.4],
            skew_ppm: 0.0,
            noise_scale: 1.0,
        }],
        actors: vec![Actor {
            room,
            position_m: [1.5, 0.0, 0.0],
            active_from_s: 0.0,
            active_to_s: 8.0,
            motion: Motion::Walk {
                axis: [1.0, 0.0, 0.0],
                amplitude_m: 0.5,
                period_s: 2.0,
                phase_rad: 0.0,
            },
            breathing: None,
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

/// Build a contract topology for a scene: one floor, one room per node room.
fn topology_for_scene(script: &SceneScript) -> Topology {
    let home = script.home.clone();
    let floor: Slug = Slug("ground".to_string());
    let firmware = FirmwareRef {
        version: "addon".to_string(),
        sha256: [0u8; 32],
    };
    let mut rooms: Vec<RoomId> = Vec::new();
    for node in &script.nodes {
        if !rooms.contains(&node.room) {
            rooms.push(node.room.clone());
        }
    }
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
    println!("USAGE:\n    aquila-addon [--port <PORT>]\n");
    println!("OPTIONS:");
    println!("    --port <PORT>   loopback control-plane port (default {DEFAULT_PORT})");
    println!("    -h, --help      print this message\n");
    println!(
        "With no options, wires the deterministic simulator into the sensing core and\n\
         serves the private loopback HTTP API."
    );
}
