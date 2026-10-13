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

use aquila_metrics::Measurement;
use aquila_sensing::{Sensing, SensingConfig};
use aquila_simulator::{Actor, Impairments, Motion, NodeSpec, Rhythm, SceneScript, SimConfig};
use aquila_types::{Band, Event, GroundTruthRecord, NodeId, Slug};

use crate::{
    count_mae, presence_f1, topology_for, CollectorSink, DeterministicCsiGenerator, FakeClock,
    Fixture, RecordingMetricSink,
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
