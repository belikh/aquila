//! The slice-1 tracer integration test.
//!
//! A deterministic scene with presence and absence spans is replayed through
//! [`ReplaySource`](aquila_sources::ReplaySource) into the real
//! [`Sensing`](aquila_sensing::Sensing) core; the core's `Event`s are captured
//! through the `CollectorSink` seam and scored against the simulator's ground
//! truth. The test asserts the v1 presence bar (F1 ≥ 0.90), determinism, and
//! that measurements were recorded — the three exit criteria of slice 1
//! (`roadmap.md`, Slice 1).

use std::net::UdpSocket;
use std::sync::{Arc, Mutex};

use aquila_metrics::{
    Authority, CommitSha, Grade, HostFingerprint, Measurement, MeasurementValue, MetricName,
    Timestamp, ToolVersions, Unit,
};
use aquila_sensing::{
    calibrate, topology_digest, CalibrationError, CalibrationExpectation, CalibrationInputs,
    CalibrationRequest, CalibrationStore, DitherSearch, DspPoseModel, HomeWindow,
    InvalidationReason, Sensing, SensingConfig, Topology,
};
use aquila_simulator::{
    Actor, Body, Impairments, Motion, NodeSpec, Rhythm, SceneScript, SimConfig,
};
use aquila_sources::{LiveConfig, NodeControl, UdpLiveSource};
use aquila_types::{
    Band, CsiEnvelope, Event, ZoneId, GroundTruthRecord, MacAddr, NodeId, Slug,
};
use aquila_wire::{AckStatus, ControlBody, ControlMessage, JoinAck, SetConfig, SyncEcho, SyncPing};

use crate::{
    count_mae, home_count_mae, home_floor_attribution_accuracy, home_presence_f1, presence_f1,
    room_attribution_accuracy, topology_for, topology_for_floors, torso_pck20, vitals_mae,
    vitals_reference, CollectorSink, DeterministicCsiGenerator, FakeClock, Fixture,
    RecordingMetricSink, VirtualNode, VirtualNodeConfig, ZonePlan, zone_count_mae,
    zone_presence_f1,
};

/// The window hop used to align predictions with ground truth, in milliseconds.
const HOP_MS: i64 = 1_000;

/// A fixed, non-zero 40-hex build identity for tests.
///
/// [`SensingConfig::validate`] refuses an empty or all-zero commit, so every
/// test that builds a core supplies this real-looking commit rather than the
/// fabricated zero hash the default used to carry (`sister-issues-core.md`
/// S15).
const TEST_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
/// The clock step between replayed frames, in nanoseconds.
///
/// Two nodes share each simulation tick, so a frame advances the clock by half
/// the tick period; one tick then spans exactly 50 ms of core time, which keeps
/// the core's window grid aligned with the simulator's ground-truth grid.
const STEP_NS: u64 = 25_000_000;

/// A deterministic two-room scene with a presence span and an absence span in
/// each room.
fn tracer_scene() -> SceneScript {
    let kitchen: Slug = Slug("kitchen".to_string());
    let lounge: Slug = Slug("lounge".to_string());
    SceneScript {
        name: "tracer".to_string(),
        home: Slug("tracer-home".to_string()),
        nodes: vec![
            NodeSpec {
                id: NodeId(1),
                zone: kitchen.clone(),
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
        actors: vec![
            Actor {
                zone: kitchen,
                position_m: [0.4, 0.0, 2.4],
                active_from_s: 0.0,
                active_to_s: 4.0,
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
            },
            Actor {
                zone: lounge,
                position_m: [0.4, 0.0, 2.4],
                active_from_s: 6.0,
                active_to_s: 12.0,
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
            },
        ],
        band: Band::B2G4,
        channel: 6,
        bandwidth_mhz: 20,
        subcarriers: 64,
        subcarrier_spacing_hz: 312_500.0,
        tick_hz: 20.0,
        duration_s: 12.0,
        noise_amplitude: 0.05,
        impairments: Impairments::default(),
    }
}

/// The observable result of one tracer run.
struct TracerRun {
    events: Vec<Event>,
    measurements: Vec<Measurement>,
    ground_truth: Vec<GroundTruthRecord>,
}

/// Replay the scene through the sensing core and collect everything it emits.
fn run_tracer() -> TracerRun {
    let script = tracer_scene();
    let sim_config = SimConfig::new(2026, Slug("tracer".to_string()), 0, 0);
    let generator =
        DeterministicCsiGenerator::new(script.clone(), sim_config).expect("valid scene");
    let fixture = Fixture::load(&generator).expect("fixture");

    let clock = Arc::new(FakeClock::new(0));
    let source = fixture.replay_source(clock.clone()).expect("replay source");
    let collector = CollectorSink::new();
    let metrics = RecordingMetricSink::new();
    let event_handle = collector.handle();
    let record_handle = metrics.handle();

    let mut sensing = Sensing::new(
        Box::new(source),
        clock.clone(),
        Box::new(collector),
        Box::new(metrics),
        None,
        None,
        topology_for(&script),
        synthetic_config(),
    )
    .expect("valid core");

    loop {
        clock.advance(STEP_NS);
        let report = sensing.step().expect("step");
        if report.ended {
            break;
        }
    }
    sensing.sink().expect("flush");

    TracerRun {
        events: snapshot(&event_handle),
        measurements: snapshot(&record_handle),
        ground_truth: fixture.ground_truth,
    }
}

/// Clone the contents of a poisoned-lock-safe mutex.
fn snapshot<T: Clone>(mutex: &Mutex<Vec<T>>) -> Vec<T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// A sensing configuration with an explicitly **declared** synthetic-fixture
/// presence baseline.
///
/// The deterministic generator is not hardware, so a test that needs presence
/// decisions declares its own baseline instead of leaning on a production
/// constant (which is now `None`: an uncalibrated core refuses to decide,
/// `sister-issues-core.md` S9). Every metric scored from these runs is graded
/// [`Authority::Synthetic`], never hardware (`sister-issues-core.md` S5).
fn synthetic_config() -> SensingConfig {
    SensingConfig {
        presence_motion_threshold: Some(0.04375),
        presence_motion_scale: 0.013125,
        measurement: aquila_sensing::MeasurementConfig::with_commit(TEST_COMMIT),
        ..SensingConfig::default()
    }
}

// ---------------------------------------------------------------------------
// Slice 2 — vitals tracer bullet (`roadmap.md`, Slice 2)
// ---------------------------------------------------------------------------

/// The clock step between replayed frames, in nanoseconds: one 20 Hz tick.
const VITALS_STEP_NS: u64 = 50_000_000;

/// A deterministic single-node scene with one stationary breathing actor.
///
/// A stationary actor is deliberate: gross motion sweeps the phase far too
/// rapidly for a vital-sign estimate, so the tracer isolates the respiratory
/// and cardiac signatures the simulator scripts.
fn vitals_scene() -> SceneScript {
    let lounge: Slug = Slug("lounge".to_string());
    SceneScript {
        name: "vitals".to_string(),
        home: Slug("vitals-home".to_string()),
        nodes: vec![NodeSpec {
            id: NodeId(1),
            zone: lounge.clone(),
            pos_m: [0.0, 0.0, 2.4],
            skew_ppm: 0.0,
            noise_scale: 1.0,
        }],
        actors: vec![Actor {
            zone: lounge,
            position_m: [0.4, 0.0, 2.4],
            active_from_s: 0.0,
            active_to_s: 90.0,
            motion: Motion::Stationary,
            breathing: Some(Rhythm {
                // 8 mm is the deep end of the spec's 3–8 mm chest
                // displacement (`capture-and-firmware.md` §7.2); a quiet
                // breather at 5 mm sits close to the estimator's SNR floor.
                amplitude_m: 0.008,
                rate_hz: 0.25,
                phase_rad: 0.0,
            }),
            heart: Some(Rhythm {
                amplitude_m: 0.0002,
                rate_hz: 1.2,
                phase_rad: 0.0,
            }),
            body: None,
        }],
        band: Band::B2G4,
        channel: 6,
        bandwidth_mhz: 20,
        subcarriers: 64,
        subcarrier_spacing_hz: 312_500.0,
        tick_hz: 20.0,
        duration_s: 90.0,
        noise_amplitude: 0.05,
        impairments: Impairments::default(),
    }
}

/// The observable result of one vitals tracer run.
struct VitalsRun {
    events: Vec<Event>,
    measurements: Vec<Measurement>,
    ground_truth: Vec<GroundTruthRecord>,
    vitals_windows: usize,
}

/// Replay the vitals scene through the sensing core and collect its output.
fn run_vitals_tracer() -> VitalsRun {
    let script = vitals_scene();
    let sim_config = SimConfig::new(4242, Slug("vitals".to_string()), 0, 0);
    let generator =
        DeterministicCsiGenerator::new(script.clone(), sim_config).expect("valid scene");
    let fixture = Fixture::load(&generator).expect("fixture");

    let clock = Arc::new(FakeClock::new(0));
    let source = fixture.replay_source(clock.clone()).expect("replay source");
    let collector = CollectorSink::new();
    let metrics = RecordingMetricSink::new();
    let event_handle = collector.handle();
    let record_handle = metrics.handle();

    let config = SensingConfig {
        vitals_enabled: true,
        vitals_window_ns: 15_000_000_000,
        presence_motion_threshold: Some(0.04375),
        presence_motion_scale: 0.013125,
        measurement: aquila_sensing::MeasurementConfig::with_commit(TEST_COMMIT),
        ..SensingConfig::default()
    };

    let mut sensing = Sensing::new(
        Box::new(source),
        clock.clone(),
        Box::new(collector),
        Box::new(metrics),
        None,
        None,
        topology_for(&script),
        config,
    )
    .expect("valid core");

    loop {
        clock.advance(VITALS_STEP_NS);
        let report = sensing.step().expect("step");
        if report.ended {
            break;
        }
    }
    sensing.sink().expect("flush");

    VitalsRun {
        events: snapshot(&event_handle),
        measurements: snapshot(&record_handle),
        ground_truth: fixture.ground_truth,
        vitals_windows: sensing.vitals_windows(),
    }
}

/// Record one ledger row into a captured measurement vector.
fn record_ledger(
    records: &Mutex<Vec<Measurement>>,
    name: &str,
    value: f64,
    unit: &str,
    samples: u64,
) {
    let measurement = Measurement {
        name: MetricName::new(name.to_string()).expect("valid metric name"),
        version: 1,
        value: MeasurementValue::Scalar(value),
        unit: Unit::new(unit).expect("valid unit"),
        samples: samples.max(1) as u32,
        seed: Some(4242),
        commit: CommitSha::new("0".repeat(40)).expect("valid commit"),
        dirty: false,
        host: HostFingerprint::new("aquila-testkit").expect("valid host"),
        tools: ToolVersions::new(),
        // Every row this helper records is measured against the deterministic
        // synthetic generator, never hardware, so its provenance is
        // `Authority::Synthetic` (`evaluation.md` §5, `sister-issues-core.md`
        // S4, S5). It is an honest synthetic-fixture number, not a real-world
        // accuracy claim.
        authority: Authority::Synthetic,
        grade: Grade::Statistical,
        timestamp: Timestamp::from_unix_ms(0),
    };
    measurement.validate().expect("valid measurement");
    records
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(measurement);
}

/// A named vitals measurement recorded by the core.
fn vitals_measurement<'a>(run: &'a VitalsRun, name: &str) -> Option<&'a Measurement> {
    run.measurements
        .iter()
        .find(|measurement| measurement.name.as_str() == name)
}

#[test]
fn breathing_mae_meets_the_v1_bar() {
    let run = run_vitals_tracer();
    let reference =
        vitals_reference(&run.ground_truth, "breathing_rate").expect("breathing ground truth");
    let heart_reference =
        vitals_reference(&run.ground_truth, "heart_rate").expect("heart ground truth");

    let (breathing_mae, breathing_n) = vitals_mae(&run.events, "breathing_rate", reference);
    let (heart_mae, heart_n) = vitals_mae(&run.events, "heart_rate", heart_reference);

    eprintln!(
        "vitals: {} vitals windows; reference breathing {reference:.2} brpm, MAE {breathing_mae:.3} brpm over {breathing_n} paired; \
heart {heart_reference:.2} bpm (experimental, ungated), MAE {heart_mae:.3} bpm over {heart_n} paired",
        run.vitals_windows
    );

    // A rate metric needs at least 30 paired windows to be graded Measured
    // (`evaluation.md` §5.1); fewer would leave it Data-gated, not gated.
    assert!(
        breathing_n >= 30,
        "only {breathing_n} paired breathing windows; the floor is 30"
    );
    assert!(
        breathing_mae <= 2.0,
        "breathing MAE {breathing_mae:.3} brpm exceeds the v1 bar of 2 brpm"
    );

    // The heart-rate result is reported honestly but not gated: if it was
    // measured it must be finite and plausible, and it is never asserted
    // against a bar (its bar is fixed from its own baseline in the ledger).
    if heart_n > 0 {
        assert!(heart_mae.is_finite(), "heart MAE must be a real number");
    }

    // Record the two slice-2 ledger rows; this test is the reproducer.
    let ledger = Mutex::new(Vec::new());
    record_ledger(
        &ledger,
        "breathing.mae",
        breathing_mae,
        "brpm",
        breathing_n as u64,
    );
    record_ledger(&ledger, "heart.mae", heart_mae, "bpm", heart_n as u64);
    let ledger = ledger.into_inner().unwrap_or_else(|p| p.into_inner());
    assert!(ledger.iter().any(|m| m.name.as_str() == "breathing.mae"));
    assert!(ledger.iter().any(|m| m.name.as_str() == "heart.mae"));
}

#[test]
fn vitals_ride_the_same_windows_as_presence() {
    let run = run_vitals_tracer();

    // Vitals windows were estimated, and the last recorded count matches the
    // running total the core reports.
    assert!(
        run.vitals_windows > 0,
        "at least one vitals window must run"
    );
    let count = vitals_measurement(&run, "sensing.vitals.window.count")
        .expect("vitals.window.count must be recorded");
    assert!(
        matches!(count.value, MeasurementValue::Count(n) if n >= 1),
        "vitals.window.count must be a positive count"
    );

    // ...on the same core clock as presence: every vitals event lands on a
    // presence window boundary (a whole second), never off-grid.
    let mut vitals_events = 0usize;
    for event in &run.events {
        if event.kind == aquila_types::EventKind::VitalAlert {
            vitals_events += 1;
            assert_eq!(
                event.t_unix_ms % HOP_MS,
                0,
                "a vitals event must fall on a presence window boundary"
            );
        }
    }
    assert!(
        vitals_events > 0,
        "at least one vitals event must be emitted"
    );

    // The frames the vitals estimator consumed are the presence frames: the
    // vitals buffer is fed from the same WindowBatch.
    let frames = vitals_measurement(&run, "sensing.vitals.window.frames")
        .expect("vitals.window.frames must be recorded");
    assert!(
        matches!(frames.value, MeasurementValue::Count(n) if n > 0),
        "the vitals buffer must have consumed real frames"
    );
}

#[test]
fn vitals_replay_is_deterministic() {
    let first = run_vitals_tracer();
    let second = run_vitals_tracer();
    assert_eq!(
        first.events, second.events,
        "same scene and seed must emit byte-identical vitals events"
    );
    assert_eq!(first.measurements, second.measurements);
    assert_eq!(first.vitals_windows, second.vitals_windows);
}

#[test]
fn vitals_readings_are_recorded() {
    let run = run_vitals_tracer();
    let mut vitals_rows = 0usize;
    for measurement in &run.measurements {
        let name = measurement.name.as_str();
        if name.starts_with("sensing.vitals.") {
            vitals_rows += 1;
        }
        measurement
            .validate()
            .expect("recorded measurement is valid");
    }
    assert!(
        vitals_rows > 0,
        "the core must record vitals Measurement rows"
    );
}

#[test]
fn presence_f1_meets_the_v1_bar() {
    let run = run_tracer();
    let score = presence_f1(&run.events, &run.ground_truth, HOP_MS);
    let mae = count_mae(&run.events, &run.ground_truth, HOP_MS);
    eprintln!(
        "tracer: TP={} FP={} FN={} TN={} precision={:.3} recall={:.3} F1={:.3} count_mae={:.3} events={} measurements={}",
        score.true_positive,
        score.false_positive,
        score.false_negative,
        score.true_negative,
        score.precision,
        score.recall,
        score.f1,
        mae,
        run.events.len(),
        run.measurements.len(),
    );
    assert!(
        !run.events.is_empty(),
        "the core must emit at least one presence event"
    );
    assert!(
        score.f1 >= 0.90,
        "presence F1 {:.3} is below the v1 bar of 0.90",
        score.f1
    );
}

#[test]
fn replay_is_deterministic() {
    let first = run_tracer();
    let second = run_tracer();
    assert_eq!(
        first.events, second.events,
        "same scene and seed must emit byte-identical events"
    );
    assert_eq!(first.measurements.len(), second.measurements.len());
}

#[test]
fn at_least_one_measurement_is_recorded() {
    let run = run_tracer();
    assert!(
        !run.measurements.is_empty(),
        "the core must record at least one Measurement"
    );
    for measurement in &run.measurements {
        measurement
            .validate()
            .expect("recorded measurement is valid");
    }
}

// ---------------------------------------------------------------------------
// Slice 3 — global whole-home calibration tracer (`roadmap.md`, Slice 3)
// ---------------------------------------------------------------------------

/// A deterministic empty-home scene with the same two-node topology as the
/// presence tracer, so the calibrated artefact binds to the same topology
/// digest.
fn calibration_empty_scene() -> SceneScript {
    let kitchen: Slug = Slug("kitchen".to_string());
    let lounge: Slug = Slug("lounge".to_string());
    SceneScript {
        // The home name matches `tracer_scene` so the topology digest is shared.
        name: "tracer".to_string(),
        home: Slug("tracer-home".to_string()),
        nodes: vec![
            NodeSpec {
                id: NodeId(1),
                zone: kitchen,
                pos_m: [0.0, 0.0, 2.4],
                skew_ppm: 12.0,
                noise_scale: 1.0,
            },
            NodeSpec {
                id: NodeId(2),
                zone: lounge,
                pos_m: [3.0, 0.0, 2.4],
                skew_ppm: -9.0,
                noise_scale: 1.0,
            },
        ],
        actors: vec![],
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

/// A walk-through scene: one actor moving in `room` for the whole capture.
fn calibration_walk_scene(zone: &ZoneId) -> SceneScript {
    let mut scene = calibration_empty_scene();
    scene.name = format!("calibration-walk-{}", zone.as_str());
    scene.actors = vec![Actor {
        zone: zone.clone(),
        position_m: [0.4, 0.0, 2.4],
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

/// Generate a scene's decoded envelopes.
fn generate_frames(script: &SceneScript, seed: u64) -> Vec<CsiEnvelope> {
    let sim_config = SimConfig::new(seed, Slug(script.name.clone()), 0, 0);
    let generator =
        DeterministicCsiGenerator::new(script.clone(), sim_config).expect("valid scene");
    generator.frames().expect("frames")
}

/// The empty-home and walk-through captures for the tracer calibration.
fn calibration_inputs() -> CalibrationInputs {
    const SEED: u64 = 2026;
    let empty = generate_frames(&calibration_empty_scene(), SEED);
    let walkthrough = [Slug("kitchen".to_string()), Slug("lounge".to_string())]
        .into_iter()
        .map(|room| {
            let frames = generate_frames(&calibration_walk_scene(&room), SEED);
            (room, frames)
        })
        .collect();
    CalibrationInputs::new(empty, walkthrough)
}

/// A process-unique temporary directory for the calibration store.
fn unique_temp_dir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("aquila-cal-{}-{n}", std::process::id()))
}

#[test]
fn calibration_tracer_calibrates_persists_and_detects() {
    // The artefact is commit-keyed, so this must match the core's commit
    // (`TEST_COMMIT`); a zero hash is now refused (S15).
    let commit = TEST_COMMIT.to_string();
    let topology = Topology::load(topology_for(&calibration_empty_scene())).expect("topology");
    let config = SensingConfig::default();
    let request = CalibrationRequest::new(commit.clone(), "cal-run-1", "2026-10-14T00:00:00Z");
    let inputs = calibration_inputs();

    // Calibrate from the synthetic empty-home + walk-through captures.
    let artifact = calibrate(&topology, &config, &request, &inputs).expect("calibration");
    eprintln!(
        "calibration {} status {:?} valid {} metrics {:?}",
        artifact.calibration_id,
        artifact.quality.status,
        artifact.quality.valid,
        artifact.quality.metrics
    );
    assert!(
        artifact.quality.valid,
        "quality gates failed: {:?}",
        artifact.quality.gates
    );

    // Reproducibility: the same fixtures reproduce the same digest.
    let again = calibrate(&topology, &config, &request, &inputs).expect("calibration");
    assert_eq!(
        again.calibration_id, artifact.calibration_id,
        "re-running on the same fixtures must reproduce the artefact digest"
    );

    // The known per-node skew injected into the fixtures is recovered and
    // inverted: node 1 is +12 ppm and node 2 is -9 ppm.
    let clock1 = artifact.clock_for(NodeId(1)).expect("node 1 clock");
    let clock2 = artifact.clock_for(NodeId(2)).expect("node 2 clock");
    assert!(
        (clock1.skew_ppm - 12.0).abs() < 0.1,
        "node 1 skew {:.4} ppm did not recover the injected 12 ppm",
        clock1.skew_ppm
    );
    assert!(
        (clock2.skew_ppm + 9.0).abs() < 0.1,
        "node 2 skew {:.4} ppm did not recover the injected -9 ppm",
        clock2.skew_ppm
    );
    // A device stamp ahead of the core (its clock runs faster) corrects to core
    // time by moving earlier: the fitted slope is below 1.
    assert!(clock1.slope < 1.0, "a fast node must fit slope < 1");
    let device_ns = 1_000_000_000_u64;
    let corrected = clock1.correct_core_ns(device_ns);
    assert!(
        (corrected - device_ns as f64).abs() < 1.0e6,
        "the clock correction must be a small, finite adjustment"
    );

    // Persist and reload.
    let dir = unique_temp_dir();
    let _ = std::fs::remove_dir_all(&dir);
    let store = CalibrationStore::new(&dir);
    store.write(&artifact).expect("write");
    let reloaded = store.read_current().expect("read").expect("present");
    // The artefact is content-addressed over its serialised body, so a
    // round-trip must preserve identity exactly (the in-memory `f64`s may
    // differ by a final bit, which does not change the canonical bytes).
    reloaded
        .verify_digest()
        .expect("digest verifies after reload");
    assert_eq!(
        reloaded.calibration_id, artifact.calibration_id,
        "the reloaded artefact must keep its content-addressed identity"
    );
    assert_eq!(
        reloaded.body_sha256, artifact.body_sha256,
        "the reloaded artefact must keep its canonical body digest"
    );
    assert_eq!(
        reloaded.to_json().expect("serialise reloaded"),
        artifact.to_json().expect("serialise original"),
        "the store must round-trip the canonical document byte-for-byte"
    );
    // Re-writing the reloaded artefact is idempotent.
    store.write(&reloaded).expect("rewrite");
    let again_loaded = store.read_current().expect("read").expect("present");
    assert_eq!(again_loaded.calibration_id, artifact.calibration_id);
    let index = store.index().expect("index");
    assert_eq!(
        index.current.as_deref(),
        Some(artifact.calibration_id.as_str()),
        "the index must point at the written artefact"
    );

    // Invalidation: a topology or commit change makes the artefact stale, and a
    // stale artefact is never silently reused.
    let digest = topology_digest(topology.model()).expect("digest");
    reloaded
        .check(&CalibrationExpectation::new(commit.clone(), digest.clone()))
        .expect("the current artefact validates");
    let moved = CalibrationExpectation::new(commit.clone(), "sha256:changed".to_string());
    match reloaded.check(&moved) {
        Err(CalibrationError::Stale(reasons)) => assert!(
            reasons
                .iter()
                .any(|r| matches!(r, InvalidationReason::TopologyChanged { .. })),
            "expected TopologyChanged, got {reasons:?}"
        ),
        other => panic!("expected a stale topology result, got {other:?}"),
    }
    let commit_changed = CalibrationExpectation::new("1".repeat(40), digest.clone());
    assert!(
        matches!(
            reloaded.check(&commit_changed),
            Err(CalibrationError::Stale(_))
        ),
        "a commit change must invalidate the artefact"
    );
    let mut model_changed = CalibrationExpectation::new(commit.clone(), digest);
    model_changed.model_versions = vec!["pose-v2".to_string()];
    assert!(
        matches!(
            reloaded.check(&model_changed),
            Err(CalibrationError::Stale(_))
        ),
        "a model change must invalidate the artefact"
    );
    let mut live_drift = std::collections::BTreeMap::new();
    live_drift.insert(1u16, 50.0_f64);
    assert!(
        reloaded
            .drift_reasons(&live_drift, 4.0, 5.0)
            .iter()
            .any(|r| matches!(r, InvalidationReason::DriftExceeded { .. })),
        "drift beyond the calibrated budget must invalidate the artefact"
    );
    assert!(
        store.load_current_validated(&moved).is_err(),
        "the store must refuse to return a stale artefact"
    );

    // Detect presence in a fresh scene using the calibrated thresholds.
    let script = tracer_scene();
    let sim_config = SimConfig::new(2026, Slug("tracer".to_string()), 0, 0);
    let generator = DeterministicCsiGenerator::new(script.clone(), sim_config).expect("scene");
    let fixture = Fixture::load(&generator).expect("fixture");

    let clock = Arc::new(FakeClock::new(0));
    let source = fixture.replay_source(clock.clone()).expect("replay source");
    let collector = CollectorSink::new();
    let metrics = RecordingMetricSink::new();
    let event_handle = collector.handle();
    let cal_config = SensingConfig {
        calibration: Some(reloaded),
        measurement: aquila_sensing::MeasurementConfig::with_commit(TEST_COMMIT),
        ..SensingConfig::default()
    };
    let mut sensing = Sensing::new(
        Box::new(source),
        clock.clone(),
        Box::new(collector),
        Box::new(metrics),
        None,
        None,
        topology_for(&script),
        cal_config,
    )
    .expect("calibrated core");
    loop {
        clock.advance(STEP_NS);
        if sensing.step().expect("step").ended {
            break;
        }
    }
    sensing.sink().expect("flush");

    let events = snapshot(&event_handle);
    let score = presence_f1(&events, &fixture.ground_truth, HOP_MS);
    eprintln!(
        "calibrated presence: TP={} FP={} FN={} TN={} precision={:.3} recall={:.3} F1={:.3}",
        score.true_positive,
        score.false_positive,
        score.false_negative,
        score.true_negative,
        score.precision,
        score.recall,
        score.f1,
    );
    assert!(
        score.f1 >= 0.90,
        "calibrated presence F1 {:.3} is below the v1 bar of 0.90",
        score.f1
    );

    // Ledger rows: the ticket's deterministic/statistical calibration rows.
    let ledger = Mutex::new(Vec::new());
    record_ledger(&ledger, "presence.f1.cross_session", score.f1, "score", 1);
    record_ledger(
        &ledger,
        "calibration.drift.per_node",
        artifact.quality.metrics.drift_residual_ms,
        "ms",
        1,
    );
    record_ledger(
        &ledger,
        "calibration.presence_f1",
        artifact.quality.metrics.presence_f1,
        "score",
        1,
    );
    {
        let digest_row = Measurement {
            name: MetricName::new("calibration.digest".to_string()).expect("valid name"),
            version: 1,
            value: MeasurementValue::Digest(artifact.calibration_id.clone()),
            unit: Unit::new("sha256").expect("valid unit"),
            samples: 1,
            seed: None,
            commit: CommitSha::new(commit.clone()).expect("valid commit"),
            dirty: false,
            host: HostFingerprint::new("aquila-testkit").expect("valid host"),
            tools: ToolVersions::new(),
            authority: Authority::Replayed,
            grade: Grade::Deterministic,
            timestamp: Timestamp::from_unix_ms(0),
        };
        digest_row.validate().expect("valid digest row");
        ledger
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(digest_row);
    }
    let ledger = ledger
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for name in [
        "calibration.digest",
        "presence.f1.cross_session",
        "calibration.drift.per_node",
    ] {
        assert!(
            ledger.iter().any(|m| m.name.as_str() == name),
            "the ledger must carry the {name} row"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stale_calibration_is_never_silently_reused() {
    let commit = "0".repeat(40);
    let topology = Topology::load(topology_for(&calibration_empty_scene())).expect("topology");
    let config = SensingConfig::default();
    let request = CalibrationRequest::new(commit, "cal-run-2", "2026-10-14T00:00:00Z");
    let artifact =
        calibrate(&topology, &config, &request, &calibration_inputs()).expect("calibration");
    assert!(artifact.quality.valid);

    // A core configured with a different commit must refuse the artefact rather
    // than sense on a stale calibration.
    let stale_config = SensingConfig {
        calibration: Some(artifact),
        measurement: aquila_sensing::MeasurementConfig {
            commit: "1".repeat(40),
            ..aquila_sensing::MeasurementConfig::default()
        },
        ..SensingConfig::default()
    };
    let script = calibration_empty_scene();
    let sim_config = SimConfig::new(2026, Slug("tracer".to_string()), 0, 0);
    let generator = DeterministicCsiGenerator::new(script.clone(), sim_config).expect("scene");
    let fixture = Fixture::load(&generator).expect("fixture");
    let clock = Arc::new(FakeClock::new(0));
    let source = fixture.replay_source(clock.clone()).expect("source");
    let outcome = Sensing::new(
        Box::new(source),
        clock,
        Box::new(CollectorSink::new()),
        Box::new(RecordingMetricSink::new()),
        None,
        None,
        topology_for(&script),
        stale_config,
    );
    assert!(
        outcome.is_err(),
        "a stale calibration must be refused, never silently reused"
    );
}

// ---------------------------------------------------------------------------
// Slice 4 — multi-node single zone: fusion and coarse pose (`roadmap.md`,
// Slice 4)
// ---------------------------------------------------------------------------

/// A deterministic, one-room, two-node scene whose single actor carries a rigid
/// coarse-pose body. Both nodes observe the actor's four torso scatterers, each
/// micro-moving at its own pose-dither frequency, so the CSP carries a
/// pose-correlated signature a principle estimator can recover.
fn pose_scene() -> SceneScript {
    let study: Slug = Slug("study".to_string());
    SceneScript {
        name: "slice4-pose".to_string(),
        home: Slug("slice4-home".to_string()),
        nodes: vec![
            NodeSpec {
                id: NodeId(1),
                zone: study.clone(),
                pos_m: [0.3, 0.3, 0.0],
                skew_ppm: 8.0,
                noise_scale: 1.0,
            },
            NodeSpec {
                id: NodeId(2),
                zone: study.clone(),
                pos_m: [0.3, 3.7, 0.0],
                skew_ppm: -6.0,
                noise_scale: 1.0,
            },
        ],
        actors: vec![Actor {
            zone: study,
            position_m: [2.0, 2.0, 0.0],
            active_from_s: 0.0,
            active_to_s: 12.0,
            motion: Motion::Stationary,
            breathing: Some(Rhythm {
                amplitude_m: 0.005,
                rate_hz: 0.25,
                phase_rad: 0.0,
            }),
            heart: None,
            body: Some(Body::default_torso(0.0)),
        }],
        band: Band::B2G4,
        channel: 6,
        bandwidth_mhz: 20,
        subcarriers: 64,
        subcarrier_spacing_hz: 312_500.0,
        tick_hz: 20.0,
        duration_s: 12.0,
        noise_amplitude: 0.05,
        impairments: Impairments::default(),
    }
}

/// The observable result of one slice-4 pose run.
struct PoseRun {
    events: Vec<Event>,
    measurements: Vec<Measurement>,
    poses: Vec<aquila_types::Pose>,
    ground_truth: Vec<GroundTruthRecord>,
    pose_truth: Vec<aquila_types::Pose>,
}

/// Replay the two-node pose scene through the sensing core with coarse pose
/// enabled, and collect everything it emits.
fn run_pose_tracer() -> PoseRun {
    run_pose_tracer_with(DitherSearch::default())
}

/// Replay the pose scene with an explicit dither search band. A band that does
/// not contain the seed-drawn dither frequencies must fail to recover the torso
/// (`sister-issues-core.md` S4): the estimator is not keyed to the simulator.
fn run_pose_tracer_with(search: DitherSearch) -> PoseRun {
    let script = pose_scene();
    let sim_config = SimConfig::new(2026, Slug("slice4-pose".to_string()), 0, 0);
    let generator =
        DeterministicCsiGenerator::new(script.clone(), sim_config).expect("valid scene");
    let fixture = Fixture::load(&generator).expect("fixture");

    let clock = Arc::new(FakeClock::new(0));
    let source = fixture.replay_source(clock.clone()).expect("replay source");
    let collector = CollectorSink::new();
    let metrics = RecordingMetricSink::new();
    let event_handle = collector.handle();
    let record_handle = metrics.handle();

    let config = SensingConfig {
        pose_enabled: true,
        // The synthetic in-domain fixture supplies no calibration artefact, so
        // the nodes are explicitly treated as calibrated here. A real
        // deployment supplies per-node calibration quality through the applied
        // artefact, and `q_n = 0` withholds pose (`fusion-sync.md` §2.3).
        pose_uncalibrated_quality: 1.0,
        pose_dither_search: search,
        presence_motion_threshold: Some(0.04375),
        presence_motion_scale: 0.013125,
        measurement: aquila_sensing::MeasurementConfig::with_commit(TEST_COMMIT),
        ..SensingConfig::default()
    };

    // The core calls the `PoseModel` seam, not the DSP directly (S8); this test
    // wires the deterministic DSP adapter as the model.
    let pose_model: Arc<dyn aquila_sensing::PoseModel> = Arc::new(DspPoseModel::new(
        config.pose_geometry,
        config.pose_dither_search,
        config.pose_min_frames,
    ));

    let mut sensing = Sensing::new(
        Box::new(source),
        clock.clone(),
        Box::new(collector),
        Box::new(metrics),
        Some(pose_model),
        None,
        topology_for(&script),
        config,
    )
    .expect("valid core");

    loop {
        clock.advance(STEP_NS);
        if sensing.step().expect("step").ended {
            break;
        }
    }
    sensing.sink().expect("flush");

    PoseRun {
        events: snapshot(&event_handle),
        measurements: snapshot(&record_handle),
        poses: sensing.poses().to_vec(),
        ground_truth: fixture.ground_truth,
        pose_truth: fixture.pose_truth,
    }
}

#[test]
fn coarse_pose_meets_the_torso_pck20_bar() {
    let run = run_pose_tracer();
    assert!(
        !run.pose_truth.is_empty(),
        "the scene must synthesise coarse-pose ground truth"
    );

    let score = torso_pck20(&run.poses, &run.pose_truth, 0.20, 1_000);
    let presence = presence_f1(&run.events, &run.ground_truth, HOP_MS);
    eprintln!(
        "slice4 pose: poses={} matched={} unmatched={} torso_pck20={:.3} (correct {}/{}) \
mean torso span {:.3} m; presence F1={:.3}, events={}, measurements={}",
        run.poses.len(),
        score.matched_poses,
        score.unmatched_predictions,
        score.pck,
        score.correct,
        score.total,
        score.torso_span_m,
        presence.f1,
        run.events.len(),
        run.measurements.len(),
    );

    assert!(
        score.total > 0,
        "at least one pose must be paired with a reference torso"
    );
    assert!(
        score.pck >= 0.65,
        "torso-PCK@20 {:.3} is below the v1 bar of 0.65",
        score.pck
    );

    // Presence must hold against the single-node baseline bar.
    assert!(
        presence.f1 >= 0.90,
        "fused presence F1 {:.3} is below the v1 bar of 0.90",
        presence.f1
    );

    // Record the slice-4 ledger row; this test is the reproducer.
    let ledger = Mutex::new(Vec::new());
    record_ledger(&ledger, "pose.pck20.torso", score.pck, "score", score.total);
    record_ledger(&ledger, "presence.f1.room", presence.f1, "score", 1);
    let ledger = ledger
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert!(ledger.iter().any(|m| m.name.as_str() == "pose.pck20.torso"));
    assert!(ledger.iter().any(|m| m.name.as_str() == "presence.f1.room"));
}

/// Whether two poses are bit-identical, treating a `NaN` coordinate as equal to
/// the same `NaN` bit pattern (the contract uses `NaN` for an absent keypoint,
/// and `NaN != NaN` under the derived `PartialEq`).
fn pose_bit_equal(a: &aquila_types::Pose, b: &aquila_types::Pose) -> bool {
    a.subject == b.subject
        && a.zone == b.zone
        && a.t_unix_ms == b.t_unix_ms
        && a.score.to_bits() == b.score.to_bits()
        && a.witness == b.witness
        && a.keys.iter().zip(&b.keys).all(|(x, y)| {
            x.id == y.id
                && x.x_m.to_bits() == y.x_m.to_bits()
                && x.y_m.to_bits() == y.y_m.to_bits()
                && x.confidence.to_bits() == y.confidence.to_bits()
                && match (x.z_m, y.z_m) {
                    (Some(zx), Some(zy)) => zx.to_bits() == zy.to_bits(),
                    (None, None) => true,
                    _ => false,
                }
        })
}

/// Whether two pose sequences are bit-identical.
fn poses_bit_equal(a: &[aquila_types::Pose], b: &[aquila_types::Pose]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| pose_bit_equal(x, y))
}

#[test]
fn pose_replay_is_deterministic() {
    let first = run_pose_tracer();
    let second = run_pose_tracer();
    assert!(
        poses_bit_equal(&first.poses, &second.poses),
        "same scene and seed must emit bit-identical coarse poses"
    );
    assert_eq!(first.events, second.events);
    assert_eq!(first.measurements.len(), second.measurements.len());
}

#[test]
fn pose_rows_are_recorded() {
    let run = run_pose_tracer();
    assert!(
        run.measurements
            .iter()
            .any(|m| m.name.as_str() == "sensing.pose.windows"),
        "the core must record a pose Measurement row"
    );
    for measurement in &run.measurements {
        measurement
            .validate()
            .expect("recorded measurement is valid");
    }
    for pose in &run.poses {
        pose.validate().expect("emitted pose is valid");
    }
}

/// Falsifiability control for S4: the estimator is not keyed to the simulator.
///
/// The pose scene draws its scatterer dither frequencies per seed from the
/// shared band `[2.5, 9.5] Hz`. Replaying the *same* scene while the estimator
/// searches a band that cannot contain those frequencies must fail to recover
/// the torso — proving the passing gated test cannot be circular.
#[test]
fn pose_search_band_is_not_keyed_to_the_simulator() {
    let correct = run_pose_tracer();
    let correct_score = torso_pck20(&correct.poses, &correct.pose_truth, 0.20, 1_000);
    assert!(
        correct_score.total > 0 && correct_score.pck >= 0.65,
        "the correct band must recover the torso: {correct_score:?}"
    );

    let wrong = run_pose_tracer_with(DitherSearch {
        lo_hz: 13.0,
        hi_hz: 14.0,
        step_hz: 0.125,
        min_separation_hz: 0.7,
    });
    let wrong_score = torso_pck20(&wrong.poses, &wrong.pose_truth, 0.20, 1_000);
    assert!(
        wrong_score.total == 0 || wrong_score.pck < 0.65,
        "a search band outside the simulated dither must not recover the torso; \
         the estimator is keyed to the simulator (pck={}, total={})",
        wrong_score.pck,
        wrong_score.total
    );
}

/// S7: the simulator's torso geometry and the estimator's fit geometry are one
/// declaration, pinned equal so they cannot drift.
#[test]
fn simulator_body_matches_the_fit_geometry() {
    let body = Body::default_torso(0.0);
    let geometry = aquila_sensing::TorsoGeometry::adult();
    assert!(
        (body.shoulder_half_width_m - geometry.shoulder_half_width_m).abs() < 1e-12
            && (body.hip_half_width_m - geometry.hip_half_width_m).abs() < 1e-12
            && (body.torso_length_m - geometry.torso_length_m).abs() < 1e-12,
        "the simulator Body geometry must equal the estimator's TorsoGeometry::adult()"
    );
    assert_eq!(
        aquila_simulator::TORSO_SCATTER_COUNT,
        aquila_sensing::TORSO_KEYPOINT_SCATTERS,
        "the torso scatterer count must be one declaration"
    );
}

// ---------------------------------------------------------------------------
// Slice 5 — floor aggregation: room → floor (`roadmap.md`, Slice 5)
// ---------------------------------------------------------------------------

/// A deterministic single-floor scene with three rooms and two nodes each
/// (`roadmap.md`, Slice 5 entry: fixtures spanning at least two rooms on one
/// floor). Three actors walk through the rooms on overlapping spans, so the
/// floor count is genuinely a sum across rooms at times.
fn floor_scene() -> SceneScript {
    let study: Slug = Slug("study".to_string());
    let kitchen: Slug = Slug("kitchen".to_string());
    let lounge: Slug = Slug("lounge".to_string());
    let node = |id: u16, zone: &Slug, x: f64| NodeSpec {
        id: NodeId(id),
        zone: zone.clone(),
        pos_m: [x, 0.0, 2.4],
        skew_ppm: 0.0,
        noise_scale: 1.0,
    };
    let walk = Motion::Walk {
        axis: [1.0, 0.0, 0.0],
        amplitude_m: 0.5,
        period_s: 2.0,
        phase_rad: 0.0,
    };
    let breath = Rhythm {
        amplitude_m: 0.005,
        rate_hz: 0.25,
        phase_rad: 0.0,
    };
    let actor = |zone: &Slug, from: f64, to: f64| {
        let pos = match zone {
            z if z == &kitchen => [5.4, 0.0, 2.4],
            z if z == &lounge => [12.4, 0.0, 2.4],
            z if z == &study => [10.4, 0.0, 2.4],
            _ => [1.5, 0.0, 2.4],
        };
        Actor {
            zone: zone.clone(),
            position_m: pos,
            active_from_s: from,
            active_to_s: to,
            motion: walk,
            breathing: Some(breath),
            heart: None,
            body: None,
        }
    };
    SceneScript {
        name: "slice5-floor".to_string(),
        home: Slug("slice5-home".to_string()),
        nodes: vec![
            node(1, &study, 0.3),
            node(2, &study, 0.5),
            node(3, &kitchen, 5.3),
            node(4, &kitchen, 5.5),
            node(5, &lounge, 7.3),
            node(6, &lounge, 7.5),
        ],
        actors: vec![
            actor(&study, 0.0, 5.0),
            actor(&kitchen, 5.0, 9.0),
            actor(&lounge, 9.0, 14.0),
        ],
        band: Band::B2G4,
        channel: 6,
        bandwidth_mhz: 20,
        subcarriers: 64,
        subcarrier_spacing_hz: 312_500.0,
        tick_hz: 20.0,
        duration_s: 14.0,
        noise_amplitude: 0.05,
        impairments: Impairments::default(),
    }
}

/// The zone → zones grouping of a scene (each zone is its own group).
fn zone_rooms_for(script: &SceneScript) -> Vec<(ZoneId, Vec<ZoneId>)> {
    let topology = topology_for(script);
    let mut map: std::collections::BTreeMap<ZoneId, Vec<ZoneId>> =
        std::collections::BTreeMap::new();
    for zone in &topology.zones {
        map.entry(zone.id.clone()).or_default().push(zone.id.clone());
    }
    map.into_iter().collect()
}

/// The clock step between replayed floor frames, in nanoseconds.
///
/// The replayed clock drives window assembly, and the six nodes of a tick are
/// pulled one frame per step, so one simulation tick spans six steps. A step of
/// ~50 ms / 6 makes one tick exactly 50 ms of core time, aligning the core's
/// one-second windows with the simulator's ground-truth grid.
const FLOOR_STEP_NS: u64 = 8_333_333;

/// The observable result of one slice-5 floor run.
struct FloorRun {
    events: Vec<Event>,
    measurements: Vec<Measurement>,
    zone_history: Vec<aquila_sensing::ZoneWindow>,
    ground_truth: Vec<GroundTruthRecord>,
}

/// Replay the multi-room single-floor scene through the sensing core and
/// collect its floor output.
fn run_floor_tracer() -> FloorRun {
    let script = floor_scene();
    let sim_config = SimConfig::new(2026, Slug("slice5-floor".to_string()), 0, 0);
    let generator =
        DeterministicCsiGenerator::new(script.clone(), sim_config).expect("valid scene");
    let fixture = Fixture::load(&generator).expect("fixture");

    let clock = Arc::new(FakeClock::new(0));
    let source = fixture.replay_source(clock.clone()).expect("replay source");
    let collector = CollectorSink::new();
    let metrics = RecordingMetricSink::new();
    let event_handle = collector.handle();
    let record_handle = metrics.handle();

    let mut sensing = Sensing::new(
        Box::new(source),
        clock.clone(),
        Box::new(collector),
        Box::new(metrics),
        None,
        None,
        topology_for(&script),
        synthetic_config(),
    )
    .expect("valid core");

    loop {
        clock.advance(FLOOR_STEP_NS);
        if sensing.step().expect("step").ended {
            break;
        }
    }
    sensing.sink().expect("flush");

    FloorRun {
        events: snapshot(&event_handle),
        measurements: snapshot(&record_handle),
        zone_history: sensing.zone_history().to_vec(),
        ground_truth: fixture.ground_truth,
    }
}

#[test]
fn floor_presence_and_count_meet_the_slice5_bars() {
    let script = floor_scene();
    let run = run_floor_tracer();
    let floor_rooms = zone_rooms_for(&script);

    let score = zone_presence_f1(&run.events, &run.ground_truth, &floor_rooms, HOP_MS);
    let (mae, count_n) = zone_count_mae(&run.events, &run.ground_truth, &floor_rooms, HOP_MS);
    let (attribution, attribution_n) =
        room_attribution_accuracy(&run.zone_history, &run.ground_truth, &floor_rooms, HOP_MS);

    eprintln!(
        "slice5 floor: TP={} FP={} FN={} TN={} precision={:.3} recall={:.3} F1={:.3}; \
floor count MAE={:.3} over {} windows; room attribution={:.3} over {} windows; events={} measurements={}",
        score.true_positive,
        score.false_positive,
        score.false_negative,
        score.true_negative,
        score.precision,
        score.recall,
        score.f1,
        mae,
        count_n,
        attribution,
        attribution_n,
        run.events.len(),
        run.measurements.len(),
    );

    assert!(
        score.f1 >= 0.90,
        "floor presence F1 {:.3} is below the v1 bar of 0.90",
        score.f1
    );
    assert!(
        mae <= 0.5,
        "floor count MAE {mae:.3} exceeds the slice-5 gate of 0.5"
    );
    assert!(
        attribution_n > 0,
        "no window had exactly one truly-occupied room, so attribution is unscoreable"
    );
    assert!(
        attribution >= 0.90,
        "room attribution accuracy {attribution:.3} is below 0.90"
    );

    // The hierarchy is data, not hard-coded: one floor holds three rooms.
    assert_eq!(floor_rooms.len(), 1);
    assert_eq!(floor_rooms[0].1.len(), 3);
    assert!(run
        .zone_history
        .iter()
        .all(|window| window.zones.len() == 1));

    // The slice-5 ledger rows; this test is the reproducer.
    let ledger = Mutex::new(Vec::new());
    record_ledger(
        &ledger,
        "presence.f1.floor",
        score.f1,
        "score",
        count_n.max(1),
    );
    record_ledger(
        &ledger,
        "attribution.accuracy.room",
        attribution,
        "score",
        attribution_n.max(1),
    );
    let ledger = ledger
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert!(ledger
        .iter()
        .any(|m| m.name.as_str() == "presence.f1.floor"));
    assert!(ledger
        .iter()
        .any(|m| m.name.as_str() == "attribution.accuracy.room"));
}

#[test]
fn floor_replay_is_deterministic() {
    let first = run_floor_tracer();
    let second = run_floor_tracer();
    assert_eq!(
        first.events, second.events,
        "same scene and seed must emit byte-identical floor events"
    );
    assert_eq!(first.measurements, second.measurements);
    assert_eq!(
        first.zone_history, second.zone_history,
        "same scene and seed must produce byte-identical zone summaries"
    );
}

#[test]
fn floor_measurements_are_recorded() {
    let run = run_floor_tracer();
    assert!(
        !run.zone_history.is_empty(),
        "at least one window must summarise the floor"
    );
    assert!(
        run.measurements
            .iter()
            .any(|m| m.name.as_str().starts_with("sensing.floor.presence.")),
        "the core must record a floor presence Measurement row"
    );
    assert!(
        run.measurements
            .iter()
            .any(|m| m.name.as_str() == "sensing.floor.count.confidence.ground"),
        "the core must record the floor count confidence"
    );
    assert!(
        run.events
            .iter()
            .all(|e| matches!(&e.scope, aquila_types::EventScope::Zone(_) | aquila_types::EventScope::Home)),
        "the core must emit at least one floor-scoped event"
    );
    for measurement in &run.measurements {
        measurement
            .validate()
            .expect("recorded measurement is valid");
    }
}

// ---------------------------------------------------------------------------
// Slice 6 — cross-floor / whole-home (`roadmap.md`, Slice 6)
// ---------------------------------------------------------------------------

/// The clock step between replayed whole-home frames, in nanoseconds.
///
/// Twelve nodes share each 20 Hz simulation tick, so one tick spans ~50 ms of
/// core time (`12 × 4_166_666 ns`), which keeps the core's one-second windows
/// aligned with the simulator's ground-truth grid.
const HOME_STEP_NS: u64 = 4_166_666;

/// A deterministic three-floor scene, two rooms per floor and two nodes per
/// room (twelve nodes, the reference ~9+ scale). Three actors walk through one
/// room on each floor on disjoint spans, so the home is occupied on one floor
/// at a time, unambiguous for floor attribution, with truly-empty gaps.
fn home_scene() -> SceneScript {
    let kitchen: Slug = Slug("kitchen".to_string());
    let lounge: Slug = Slug("lounge".to_string());
    let study: Slug = Slug("study".to_string());
    let bedzone: Slug = Slug("bedroom".to_string());
    let loft: Slug = Slug("loft".to_string());
    let landing: Slug = Slug("landing".to_string());

    let node = |id: u16, zone: &Slug, x: f64| NodeSpec {
        id: NodeId(id),
        zone: zone.clone(),
        pos_m: [x, 0.0, 2.4],
        skew_ppm: 0.0,
        noise_scale: 1.0,
    };
    let walk = Motion::Walk {
        axis: [1.0, 0.0, 0.0],
        amplitude_m: 0.5,
        period_s: 2.0,
        phase_rad: 0.0,
    };
    let breath = Rhythm {
        amplitude_m: 0.005,
        rate_hz: 0.25,
        phase_rad: 0.0,
    };
        let actor = |zone: &Slug, from: f64, to: f64| {
        let pos = match zone {
            z if z == &kitchen => [0.4, 0.0, 2.4],
            z if z == &lounge => [5.4, 0.0, 2.4],
            z if z == &study => [10.4, 0.0, 2.4],
            z if z == &loft => [15.4, 0.0, 2.4],
            z if z == &landing => [20.4, 0.0, 2.4],
            _ => [1.5, 0.0, 2.4],
        };
        Actor {
            zone: zone.clone(),
            position_m: pos,
            active_from_s: from,
            active_to_s: to,
            motion: walk,
            breathing: Some(breath),
            heart: None,
            body: None,
        }
    };

    SceneScript {
        name: "slice6-home".to_string(),
        home: Slug("slice6-home".to_string()),
        nodes: vec![
            node(1, &kitchen, 0.3),
            node(2, &kitchen, 3.7),
            node(3, &lounge, 0.3),
            node(4, &lounge, 3.7),
            node(5, &study, 0.3),
            node(6, &study, 3.7),
            node(7, &study, 0.3),
            node(8, &study, 3.7),
            node(9, &loft, 0.3),
            node(10, &loft, 3.7),
            node(11, &landing, 0.3),
            node(12, &landing, 3.7),
        ],
        actors: vec![
            actor(&kitchen, 0.0, 6.0),
            actor(&study, 8.0, 14.0),
            actor(&loft, 16.0, 22.0),
        ],
        band: Band::B2G4,
        channel: 6,
        bandwidth_mhz: 20,
        subcarriers: 64,
        subcarrier_spacing_hz: 312_500.0,
        tick_hz: 20.0,
        duration_s: 24.0,
        noise_amplitude: 0.05,
        impairments: Impairments::default(),
    }
}

/// The three-zone plan for [`home_scene`]: ground/first/second, two zones each.
fn home_zone_plans() -> Vec<ZonePlan> {
    vec![
        ZonePlan {
            id: Slug("ground".to_string()),
            name: "Ground".to_string(),
            zones: vec![Slug("kitchen".to_string()), Slug("lounge".to_string())],
        },
        ZonePlan {
            id: Slug("first".to_string()),
            name: "First".to_string(),
            zones: vec![Slug("study".to_string()), Slug("bedroom".to_string())],
        },
        ZonePlan {
            id: Slug("second".to_string()),
            name: "Second".to_string(),
            zones: vec![Slug("loft".to_string()), Slug("landing".to_string())],
        },
    ]
}

/// The zone → zones grouping of a scene, read from its multi-zone topology.
fn home_zone_rooms(script: &SceneScript) -> Vec<(ZoneId, Vec<ZoneId>)> {
    let topology = topology_for_floors(script, &home_zone_plans());
    let mut map: std::collections::BTreeMap<ZoneId, Vec<ZoneId>> =
        std::collections::BTreeMap::new();
    for zone in &topology.zones {
        map.entry(zone.id.clone()).or_default().push(zone.id.clone());
    }
    map.into_iter().collect()
}

/// The observable result of one slice-6 whole-home run.
struct HomeRun {
    events: Vec<Event>,
    measurements: Vec<Measurement>,
    home_history: Vec<HomeWindow>,
    ground_truth: Vec<GroundTruthRecord>,
}

/// Replay the three-floor scene through the sensing core and collect its
/// whole-home output.
fn run_home_tracer() -> HomeRun {
    let script = home_scene();
    let sim_config = SimConfig::new(2026, Slug("slice6-home".to_string()), 0, 0);
    let generator =
        DeterministicCsiGenerator::new(script.clone(), sim_config).expect("valid scene");
    let fixture = Fixture::load(&generator).expect("fixture");

    let clock = Arc::new(FakeClock::new(0));
    let source = fixture.replay_source(clock.clone()).expect("replay source");
    let collector = CollectorSink::new();
    let metrics = RecordingMetricSink::new();
    let event_handle = collector.handle();
    let record_handle = metrics.handle();

    let mut sensing = Sensing::new(
        Box::new(source),
        clock.clone(),
        Box::new(collector),
        Box::new(metrics),
        None,
        None,
        topology_for_floors(&script, &home_zone_plans()),
        synthetic_config(),
    )
    .expect("valid core");

    loop {
        clock.advance(HOME_STEP_NS);
        if sensing.step().expect("step").ended {
            break;
        }
    }
    sensing.sink().expect("flush");

    HomeRun {
        events: snapshot(&event_handle),
        measurements: snapshot(&record_handle),
        home_history: sensing.home_history().to_vec(),
        ground_truth: fixture.ground_truth,
    }
}

#[test]
fn whole_home_presence_count_and_attribution_meet_the_slice6_bars() {
    let script = home_scene();
    let run = run_home_tracer();
    let floor_rooms = home_zone_rooms(&script);

    let presence = home_presence_f1(&run.events, &run.ground_truth, HOP_MS);
    let (count_mae_home, count_n) = home_count_mae(&run.events, &run.ground_truth, HOP_MS);
    let (attribution, attribution_n) =
        home_floor_attribution_accuracy(&run.home_history, &run.ground_truth, &floor_rooms, HOP_MS);

    eprintln!(
        "slice6 home: TP={} FP={} FN={} TN={} precision={:.3} recall={:.3} F1={:.3}; \
home count MAE={:.3} over {} windows; floor attribution={:.3} over {} windows; events={} measurements={}",
        presence.true_positive,
        presence.false_positive,
        presence.false_negative,
        presence.true_negative,
        presence.precision,
        presence.recall,
        presence.f1,
        count_mae_home,
        count_n,
        attribution,
        attribution_n,
        run.events.len(),
        run.measurements.len(),
    );

    // The three-floor entry criterion: at least two rooms on each of three
    // floors, read from the topology (data, not hard-coded).
    assert_eq!(floor_rooms.len(), 3, "the home must span three floors");
    assert!(
        floor_rooms.iter().all(|(_, rooms)| rooms.len() >= 2),
        "every floor must hold at least two rooms"
    );
    assert_eq!(script.nodes.len(), 12, "the reference ~9+ node scale");

    // The home presence gate: F1 ≥ 0.90 at reference scale.
    assert!(
        presence.f1 >= 0.90,
        "whole-home presence F1 {:.3} is below the v1 bar of 0.90",
        presence.f1
    );
    // Count is recorded and within the settled band (not gated further).
    assert!(
        count_mae_home <= 0.5,
        "whole-home count MAE {count_mae_home:.3} exceeds the band of 0.5"
    );
    // Floor attribution is correct when exactly one floor is truly occupied.
    assert!(
        attribution_n > 0,
        "no window had exactly one truly-occupied floor, so attribution is unscoreable"
    );
    assert!(
        attribution >= 0.90,
        "whole-home floor attribution {attribution:.3} is below 0.90"
    );

    // The home must have produced a summary each window, with a convolved count.
    assert!(run.home_history.iter().all(|w| w.home.count.is_some()));

    // The slice-6 ledger rows; this test is the reproducer.
    let ledger = Mutex::new(Vec::new());
    record_ledger(
        &ledger,
        "presence.f1.home",
        presence.f1,
        "score",
        count_n.max(1),
    );
    record_ledger(
        &ledger,
        "count.mae.home",
        count_mae_home,
        "people",
        count_n.max(1),
    );
    record_ledger(
        &ledger,
        "attribution.accuracy.floor",
        attribution,
        "score",
        attribution_n.max(1),
    );
    let ledger = ledger
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for name in [
        "presence.f1.home",
        "count.mae.home",
        "attribution.accuracy.floor",
    ] {
        assert!(
            ledger.iter().any(|m| m.name.as_str() == name),
            "the ledger must carry the {name} row"
        );
    }
}

#[test]
fn home_replay_is_deterministic() {
    let first = run_home_tracer();
    let second = run_home_tracer();
    assert_eq!(
        first.events, second.events,
        "same scene and seed must emit byte-identical whole-home events"
    );
    assert_eq!(first.measurements, second.measurements);
    assert_eq!(
        first.home_history, second.home_history,
        "same scene and seed must produce byte-identical whole-home summaries"
    );
}

#[test]
fn home_measurements_are_recorded() {
    let run = run_home_tracer();
    assert!(
        !run.home_history.is_empty(),
        "at least one window must summarise the whole home"
    );
    for name in [
        "sensing.home.presence",
        "sensing.home.count",
        "sensing.home.attribution",
    ] {
        assert!(
            run.measurements.iter().any(|m| m.name.as_str() == name),
            "the core must record the {name} Measurement row"
        );
    }
    assert!(
        run.events
            .iter()
            .any(|e| matches!(&e.scope, aquila_types::EventScope::Home)),
        "the core must emit at least one home-scoped event"
    );
    for measurement in &run.measurements {
        measurement
            .validate()
            .expect("recorded measurement is valid");
    }
    for window in &run.home_history {
        assert!(window.home.count.is_some());
    }
}

// ---------------------------------------------------------------------------
// Slice 7 — live nodes and the node control channel (`roadmap.md`, Slice 7)
// ---------------------------------------------------------------------------

/// The observable result of one live tracer run.
struct LiveRun {
    events: Vec<Event>,
    measurements: Vec<Measurement>,
    ground_truth: Vec<GroundTruthRecord>,
    streams: usize,
    stats: aquila_sources::LiveStatsSnapshot,
}

/// Drive virtual nodes over loopback UDP into `UdpLiveSource` and the sensing
/// core, and collect everything the core emits.
///
/// The data path is deterministic: the virtual nodes send one frame per scene
/// node per tick in the same order the replay tracer does, and the core's
/// injected [`FakeClock`] advances one 25 ms step per frame, so the live run is
/// byte-identical to a `Deterministic` replay of the same scene.
fn run_live_tracer() -> LiveRun {
    let script = tracer_scene();
    let clock = Arc::new(FakeClock::new(0));

    let source = UdpLiveSource::bind(
        "127.0.0.1:0".parse().expect("loopback address"),
        clock.clone(),
        LiveConfig::default(),
    )
    .expect("bind live source");
    let data_addr = source.local_addr().expect("live source address");
    let stats = source.stats();

    let core_control = NodeControl::bind_any().expect("core control socket");
    let core_control_addr = core_control.local_addr().expect("core control address");

    let mut nodes: Vec<VirtualNode> = script
        .nodes
        .iter()
        .map(|spec| {
            let mac = MacAddr([0, 0, 0, 0, 0, spec.id.0 as u8]);
            let config =
                VirtualNodeConfig::new(spec.id, mac, 1, 0x1234, data_addr, core_control_addr);
            let sim = SimConfig::new(2026, Slug("tracer".to_string()), 0, 0);
            VirtualNode::new(script.clone(), sim, config).expect("virtual node")
        })
        .collect();
    let streams = nodes.len();

    let collector = CollectorSink::new();
    let metrics = RecordingMetricSink::new();
    let event_handle = collector.handle();
    let record_handle = metrics.handle();

    let mut sensing = Sensing::new(
        Box::new(source),
        clock.clone(),
        Box::new(collector),
        Box::new(metrics),
        None,
        None,
        topology_for(&script),
        synthetic_config(),
    )
    .expect("valid core");

    loop {
        let mut sent = 0usize;
        for node in &mut nodes {
            if node.send_next().expect("send frame").is_some() {
                sent += 1;
            }
        }
        if sent == 0 {
            break;
        }
        for _ in 0..sent {
            clock.advance(STEP_NS);
            sensing.step().expect("step");
        }
    }
    sensing.sink().expect("flush");

    let ground_truth = DeterministicCsiGenerator::new(
        script.clone(),
        SimConfig::new(2026, Slug("tracer".to_string()), 0, 0),
    )
    .expect("generator")
    .ground_truth()
    .into_iter()
    .map(|(_, record)| record)
    .collect();

    LiveRun {
        events: snapshot(&event_handle),
        measurements: snapshot(&record_handle),
        ground_truth,
        streams,
        stats: stats.snapshot(),
    }
}

#[test]
fn live_presence_f1_meets_the_v1_bar() {
    let run = run_live_tracer();
    let score = presence_f1(&run.events, &run.ground_truth, HOP_MS);
    eprintln!(
        "slice7 live: TP={} FP={} FN={} TN={} precision={:.3} recall={:.3} F1={:.3}; \
datagrams={} decoded={} bad_frame={} bad_version={} streams={} events={} measurements={}",
        score.true_positive,
        score.false_positive,
        score.false_negative,
        score.true_negative,
        score.precision,
        score.recall,
        score.f1,
        run.stats.datagrams,
        run.stats.decoded,
        run.stats.bad_frame,
        run.stats.bad_version,
        run.streams,
        run.events.len(),
        run.measurements.len(),
    );

    assert!(
        !run.events.is_empty(),
        "the core must emit presence events from live UDP frames"
    );
    assert!(
        score.f1 >= 0.90,
        "live presence F1 {:.3} is below the v1 bar of 0.90",
        score.f1
    );
    assert!(
        run.stats.decoded > 0,
        "the live source must decode real AQCI datagrams"
    );
    assert_eq!(run.stats.bad_frame, 0, "no live datagram may be malformed");

    // The slice-7 ledger rows; this test is the reproducer.
    let ledger = Mutex::new(Vec::new());
    record_ledger(&ledger, "live.presence.f1", score.f1, "score", 1);
    record_ledger(
        &ledger,
        "host.streams.count",
        run.streams as f64,
        "count",
        run.streams as u64,
    );
    let ledger = ledger
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert!(ledger.iter().any(|m| m.name.as_str() == "live.presence.f1"));
    assert!(ledger
        .iter()
        .any(|m| m.name.as_str() == "host.streams.count"));
}

#[test]
fn live_replay_is_deterministic() {
    let first = run_live_tracer();
    let second = run_live_tracer();
    assert_eq!(
        first.events, second.events,
        "same scene and seed must emit byte-identical live events"
    );
    assert_eq!(first.measurements.len(), second.measurements.len());
    assert_eq!(first.stats.decoded, second.stats.decoded);
}

/// A representative `SET_CONFIG` body at `config_version`.
fn set_config_body(config_version: u32) -> SetConfig {
    SetConfig {
        config_version,
        config_hash: [2u8; 32],
        channel: 11,
        bandwidth_mhz: 20,
        secondary: 0,
        csi_enable: true,
        lltf_enable: true,
        ht_ltf_enable: true,
        stbc_ht_ltf_enable: false,
        csi_len_cap: 612,
        max_tx_rate_hz: 100,
        heartbeat_ms: 5_000,
        source_mac_allowlist: vec![],
    }
}

#[test]
fn node_control_full_round_trip_succeeds() {
    let script = tracer_scene();
    let dummy = UdpSocket::bind("127.0.0.1:0").expect("dummy data sink");
    let core_data = dummy.local_addr().expect("dummy address");
    let mut core = NodeControl::bind_any().expect("core control socket");
    let core_control = core.local_addr().expect("core control address");
    let mac = MacAddr([0, 0, 0, 0, 0, 1]);
    let config = VirtualNodeConfig::new(NodeId(1), mac, 7, 0xABCD, core_data, core_control);
    let sim = SimConfig::new(2026, Slug("tracer".to_string()), 0, 0);
    let mut node = VirtualNode::new(script, sim, config).expect("virtual node");
    let node_control = node.control_addr().expect("node control address");

    // 1. Join/pair: node-initiated.
    let join_id = node.send_join_request().expect("join request");
    let (request, from) = core.recv().expect("join recv");
    assert_eq!(request.request_id, join_id);
    match &request.body {
        ControlBody::JoinRequest(body) => {
            assert_eq!(body.node, mac);
            assert_eq!(body.pairing_code, 0xABCD);
        }
        other => panic!("expected JoinRequest, got {other:?}"),
    }
    let token = [9u8; 32];
    let ack = ControlMessage {
        request_id: request.request_id,
        body: ControlBody::JoinAck(JoinAck {
            node: mac,
            token,
            config_version: 3,
            config_hash: [1u8; 32],
        }),
    };
    core.send(from, &ack).expect("join ack");
    assert!(node.serve_control_once().expect("serve ack"));
    assert!(node.is_paired(), "the node must be paired after JOIN_ACK");
    assert_eq!(node.token(), token);
    assert_eq!(node.config_version(), 3);

    // 2. GetConfig: core-initiated, answered with a RESULT.
    let get = ControlMessage {
        request_id: 100,
        body: ControlBody::GetConfig,
    };
    core.send(node_control, &get).expect("get config");
    assert!(node.serve_control_once().expect("serve get config"));
    let (reply, _) = core.recv().expect("get config reply");
    assert_eq!(reply.request_id, 100);
    assert!(matches!(reply.body, ControlBody::Result(ref a) if a.status == AckStatus::Active));

    // 3. SetConfig: applied transactionally.
    let set = ControlMessage {
        request_id: 101,
        body: ControlBody::SetConfig(set_config_body(4)),
    };
    core.send(node_control, &set).expect("set config");
    assert!(node.serve_control_once().expect("serve set config"));
    let (reply, _) = core.recv().expect("set config reply");
    assert_eq!(reply.request_id, 101);
    assert!(matches!(reply.body, ControlBody::Result(ref a) if a.status == AckStatus::Active));
    assert_eq!(node.config_version(), 4);

    // A stale config version is rejected, not applied (idempotent, versioned).
    let stale = ControlMessage {
        request_id: 102,
        body: ControlBody::SetConfig(set_config_body(2)),
    };
    core.send(node_control, &stale).expect("stale config");
    assert!(node.serve_control_once().expect("serve stale config"));
    let (reply, _) = core.recv().expect("stale config reply");
    assert!(matches!(reply.body, ControlBody::Result(ref a) if a.status == AckStatus::Rejected));
    assert_eq!(
        node.config_version(),
        4,
        "a stale config must not be applied"
    );

    // 4. Status: node-initiated heartbeat.
    let status_id = node.send_status().expect("status");
    let (status, _) = core.recv().expect("status recv");
    assert_eq!(status.request_id, status_id);
    assert!(matches!(status.body, ControlBody::Status(_)));

    // 5. SyncPing/Echo: the time-sync round trip.
    let ping = ControlMessage {
        request_id: 200,
        body: ControlBody::SyncPing(SyncPing { t1_us: 111 }),
    };
    core.send(node_control, &ping).expect("sync ping");
    assert!(node.serve_control_once().expect("serve sync ping"));
    let (echo, _) = core.recv().expect("sync echo recv");
    assert_eq!(echo.request_id, 200);
    match echo.body {
        ControlBody::SyncEcho(SyncEcho {
            t1_us,
            t2_us,
            t3_us,
        }) => {
            assert_eq!(t1_us, 111, "the echo must carry the ping's t1");
            assert!(t3_us >= t2_us, "the node send time cannot precede receive");
        }
        other => panic!("expected SyncEcho, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// ADR-0005 — the int8 → f32 derivation, end to end
// ---------------------------------------------------------------------------

/// Prove the whole int8 I/Q path is exercised: a simulator frame is quantised
/// to raw signed int8, framed on the `AQCI` wire, decoded, and then the core
/// derives the complex bins — the derivation the firmware used to do on-node.
#[test]
fn int8_iq_derivation_is_exercised_end_to_end() {
    use aquila_simulator::SimulatorSource;
    use aquila_sources::{CsiSource, FrameOrEnd};
    use aquila_wire::{decode_datagram, encode_datagram};

    let script = tracer_scene();
    let sim_config = SimConfig::new(2026, Slug("iq-derivation".to_string()), 0, 0);
    let mut source = SimulatorSource::new(script, sim_config).expect("simulator");
    let frame = match source.recv().expect("recv") {
        FrameOrEnd::Frame(envelope) => envelope.frame,
        FrameOrEnd::End => panic!("the scene must yield at least one frame"),
    };

    // The frame carries the raw sample verbatim: whole I/Q pairs, every count a
    // legal int8, and a non-trivial spread of levels.
    frame.validate().expect("valid frame");
    assert_eq!(frame.i_q.len(), 2 * frame.n_subcarriers());
    assert!(!frame.i_q.is_empty());
    let distinct: std::collections::BTreeSet<i8> = frame.i_q.iter().copied().collect();
    assert!(
        distinct.len() > 8,
        "the int8 path must be genuinely exercised, got {} levels",
        distinct.len()
    );

    // Simulator -> wire -> decode: the raw counts survive the codec unchanged.
    let bytes = encode_datagram(&frame);
    let decoded = decode_datagram(&bytes).expect("decode");
    assert_eq!(decoded, frame);

    // Decode: the core derives the complex bins from the int8 sample.
    let complex = aquila_signal::decode_csi(&decoded).expect("decode_csi");
    assert_eq!(complex.bins.len(), decoded.n_subcarriers());
    let full_scale = aquila_types::IQ_COUNTS_PER_UNIT;
    for k in 0..decoded.n_subcarriers() {
        // Imaginary-first: the first byte of each pair is Q, the second is I.
        let q = f32::from(decoded.i_q[2 * k]) / full_scale;
        let i = f32::from(decoded.i_q[2 * k + 1]) / full_scale;
        assert!((complex.bins[k].re - i).abs() < 1e-6);
        assert!((complex.bins[k].im - q).abs() < 1e-6);
    }

    // The derived amplitude and phase are the magnitude and argument of the
    // same int8 sample; nothing was stored on the node.
    let amplitude = decoded.amplitude();
    let phase = decoded.phase();
    for k in 0..decoded.n_subcarriers() {
        let q = f32::from(decoded.i_q[2 * k]) / full_scale;
        let i = f32::from(decoded.i_q[2 * k + 1]) / full_scale;
        assert!((amplitude[k] - (i * i + q * q).sqrt()).abs() < 1e-6);
        assert!((phase[k] - q.atan2(i)).abs() < 1e-6);
    }
}

/// The I/Q order is imaginary-first: a known complex sample survives the wire
/// and decodes to a [`ComplexFrame`](aquila_signal::ComplexFrame) whose phase
/// **sign** is the one the sample encodes.
///
/// The sample is I = +64, Q = −64 (phase −π/4). Stored imaginary-first it is the
/// two bytes `[Q, I] = [−64, +64]`. If the codec or the decoder assumed
/// real-first, the same bytes would decode as I = −64, Q = +64, a phase of
/// +3π/4, so asserting the sign of the phase fails loudly on an order swap.
#[test]
fn phase_sign_is_imaginary_first_end_to_end() {
    use aquila_types::{CsiFrame, PhyFlags};
    use aquila_wire::{decode_datagram, encode_datagram};

    let frame = CsiFrame {
        node_id: NodeId(1),
        boot_epoch: 3,
        sequence: 0,
        device_time_us: 0,
        band: Band::B2G4,
        channel: 6,
        bandwidth_mhz: 20,
        subcarrier_offset: 0,
        // Imaginary-first interleaved sample: [Q0, I0].
        i_q: vec![-64, 64],
        rssi_dbm: -40,
        noise_floor_dbm: -90,
        phy: PhyFlags(PhyFlags::HT),
        first_word_invalid: false,
    };
    frame.validate().expect("valid frame");

    // The full round trip: raw sample → AQCI wire → contract frame → core bins.
    let decoded = decode_datagram(&encode_datagram(&frame)).expect("wire round trip");
    assert_eq!(
        decoded, frame,
        "the raw sample must survive the wire verbatim"
    );

    let complex = aquila_signal::decode_csi(&decoded).expect("decode_csi");
    assert_eq!(complex.bins.len(), 1);
    let full_scale = aquila_types::IQ_COUNTS_PER_UNIT;
    assert!((complex.bins[0].re - 64.0 / full_scale).abs() < 1e-6);
    assert!((complex.bins[0].im - (-64.0 / full_scale)).abs() < 1e-6);

    let phase = decoded.phase();
    assert!(
        phase[0] < 0.0,
        "imaginary-first I=+64, Q=−64 must have a negative phase, got {}",
        phase[0]
    );
    assert!(
        (phase[0] - (-core::f32::consts::FRAC_PI_4)).abs() < 1e-6,
        "expected −π/4, got {}",
        phase[0]
    );
}
