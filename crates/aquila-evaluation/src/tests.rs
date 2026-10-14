//! `aquila-evaluation` tests: a simulated run is scored end to end.

use std::sync::Arc;

use aquila_metrics::InMemoryMetricSink;
use aquila_sensing::{ManualClock, Sensing, SensingConfig};
use aquila_simulator::{
    Actor, Impairments, Motion, NodeSpec, SceneScript, SimConfig, SimulatorSource,
};
use aquila_types::{
    Band, DeviceType, FirmwareRef, Floor, Home, MacAddr, Node, NodeId, Room, RoomId, RoomKind, Slug,
    Topology, CONTRACT_VERSION,
};

use crate::{CollectorSink, EvalScope, FixtureGroundTruth, GroundTruth, Harness};

fn slug(value: &str) -> Slug {
    Slug(value.to_string())
}

/// A deterministic single-node scene with one swaying actor present throughout.
fn scene() -> SceneScript {
    let lounge: RoomId = slug("lounge");
    SceneScript {
        name: "eval".to_string(),
        home: slug("eval-home"),
        nodes: vec![NodeSpec {
            id: NodeId(1),
            room: lounge.clone(),
            pos_m: [0.0, 0.0, 2.4],
            skew_ppm: 0.0,
            noise_scale: 1.0,
        }],
        actors: vec![Actor {
            room: lounge,
            position_m: [1.5, 0.0, 0.0],
            active_from_s: 0.0,
            active_to_s: 20.0,
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
        duration_s: 20.0,
        noise_amplitude: 0.05,
        impairments: Impairments::default(),
    }
}

/// A minimal one-floor topology for a scene, mirroring the simulator's needs.
fn topology_for_scene(script: &SceneScript) -> Topology {
    let home = script.home.clone();
    let floor = slug("ground");
    let firmware = FirmwareRef {
        version: "eval".to_string(),
        sha256: None,
    };
    let mut rooms: Vec<RoomId> = Vec::new();
    for node in &script.nodes {
        if !rooms.contains(&node.room) {
            rooms.push(node.room.clone());
        }
    }
    Topology {
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
    }
}

#[test]
fn scores_a_simulated_run() {
    let script = scene();
    let config = SimConfig::new(7, slug("eval"), 0, 0);

    // The reference is synthesised from the same scene, so this is the
    // replay-and-simulation-first path (`evaluation.md` §1).
    let truth = FixtureGroundTruth::presence_from_scene(&script, &config).expect("reference");
    assert_eq!(truth.kind(), crate::ReferenceKind::Presence);
    assert!(!truth.is_empty());

    let source = SimulatorSource::new(script.clone(), config.clone()).expect("simulator");
    let clock = Arc::new(ManualClock::new(0));
    let collector = CollectorSink::new();
    let metrics = InMemoryMetricSink::new();

    let mut sensing = Sensing::new(
        Box::new(source),
        clock,
        Box::new(collector.clone()),
        Box::new(metrics),
        None,
        None,
        topology_for_scene(&script),
        SensingConfig {
            // A declared synthetic-fixture baseline, so the deterministic
            // generator drives presence decisions (`sister-issues-core.md` S9).
            presence_motion_threshold: Some(0.04375),
            presence_motion_scale: 0.013125,
            ..SensingConfig::default()
        },
    )
    .expect("valid core");

    loop {
        let report = sensing.step().expect("step");
        if report.ended {
            break;
        }
    }
    sensing.sink().expect("flush");

    let events = collector.events();
    assert!(!events.is_empty(), "the core must emit at least one event");

    let scope = EvalScope {
        sessions: vec![config.session.clone()],
        rooms: vec![slug("lounge")],
    };
    let report = Harness::new()
        .run(&truth, &config.session, &events, &scope)
        .expect("scored");

    assert!(report.paired_samples > 0, "must pair at least one sample");
    assert!(report.provenance.records > 0);
    assert!(report.coverage.fraction() > 0.0);
    if let Some(f1) = report.presence_f1 {
        assert!((0.0..=1.0).contains(&f1), "F1 {f1} out of range");
    }
}

#[test]
fn zero_paired_samples_is_not_measured() {
    let script = scene();
    let config = SimConfig::new(7, slug("eval"), 0, 0);
    // A reference for a different session cannot pair with any event.
    let truth = FixtureGroundTruth::presence_from_scene(&script, &config).expect("reference");
    let other = slug("some-other-session");
    let error = Harness::new()
        .run(&truth, &other, &[], &EvalScope::default())
        .expect_err("no samples");
    assert!(matches!(error, crate::EvaluationError::NoSamples(_)));
}
