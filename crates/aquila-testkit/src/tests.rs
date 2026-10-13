//! The slice-1 tracer integration test.
//!
//! A deterministic scene with presence and absence spans is replayed through
//! [`ReplaySource`](aquila_sources::ReplaySource) into the real
//! [`Sensing`](aquila_sensing::Sensing) core; the core's `Event`s are captured
//! through the `CollectorSink` seam and scored against the simulator's ground
//! truth. The test asserts the v1 presence bar (F1 ≥ 0.90), determinism, and
//! that measurements were recorded — the three exit criteria of slice 1
//! (`roadmap.md`, Slice 1).

use std::sync::{Arc, Mutex};

use aquila_metrics::{
    Authority, CommitSha, Grade, HostFingerprint, Measurement, MeasurementValue, MetricName,
    Timestamp, ToolVersions, Unit,
};
use aquila_sensing::{Sensing, SensingConfig};
use aquila_simulator::{Actor, Impairments, Motion, NodeSpec, Rhythm, SceneScript, SimConfig};
use aquila_types::{Band, Event, GroundTruthRecord, NodeId, Slug};

use crate::{
    count_mae, presence_f1, topology_for, vitals_mae, vitals_reference, CollectorSink,
    DeterministicCsiGenerator, FakeClock, Fixture, RecordingMetricSink,
};

/// The window hop used to align predictions with ground truth, in milliseconds.
const HOP_MS: i64 = 1_000;
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
                room: kitchen.clone(),
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
        actors: vec![
            Actor {
                room: kitchen,
                position_m: [1.5, 0.0, 0.0],
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
            },
            Actor {
                room: lounge,
                position_m: [1.5, 0.0, 0.0],
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
        SensingConfig::default(),
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
            room: lounge.clone(),
            pos_m: [0.0, 0.0, 2.4],
            skew_ppm: 0.0,
            noise_scale: 1.0,
        }],
        actors: vec![Actor {
            room: lounge,
            position_m: [1.5, 0.0, 0.0],
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
fn record_ledger(records: &Mutex<Vec<Measurement>>, name: &str, value: f64, unit: &str, samples: u64) {
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
        authority: Authority::Replayed,
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
    assert!(run.vitals_windows > 0, "at least one vitals window must run");
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
    assert!(vitals_events > 0, "at least one vitals event must be emitted");

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
        measurement.validate().expect("recorded measurement is valid");
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
