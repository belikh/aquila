//! Tests for the deterministic simulator and its interlock with the pipeline.
//!
//! The interlock test is the point: a synthesised session written by
//! [`synthesise_session`] must decode back, through the real `aquila-wire`
//! codec, to the very frames the simulator produced directly.

use std::sync::Arc;

use aquila_session::{verify_bytes, SessionReader};
use aquila_sources::{CsiSource, FrameOrEnd, ReplayMode, ReplaySource};
use aquila_types::{Clock, CoreTime, CsiEnvelope, CsiFrame, EventPayload, NodeId, Slug};

use crate::*;

/// A frozen clock for deterministic replay.
#[derive(Debug)]
struct FixedClock(u64);

impl Clock for FixedClock {
    fn now(&self) -> CoreTime {
        CoreTime(self.0)
    }
}

/// A fixed simulation configuration for a scene.
fn config(seed: u64) -> SimConfig {
    SimConfig::new(seed, Slug("sim-fixture".into()), 1_700_000_000_000, 0)
}

/// Drain a source into its decoded envelopes.
fn drain(source: &mut dyn CsiSource) -> Vec<CsiEnvelope> {
    let mut out = Vec::new();
    while let Ok(FrameOrEnd::Frame(env)) = source.recv() {
        out.push(env);
    }
    out
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

#[test]
fn same_seed_produces_identical_session_bytes() {
    let script = SceneScript::demo();
    let a = synthesise_session(&script, &config(1234)).unwrap();
    let b = synthesise_session(&script, &config(1234)).unwrap();
    assert_eq!(a, b, "same seed must produce byte-identical sessions");
    assert!(a.starts_with(b"AQS"));
}

#[test]
fn different_seed_produces_different_bytes() {
    let script = SceneScript::demo();
    let a = synthesise_session(&script, &config(1)).unwrap();
    let b = synthesise_session(&script, &config(2)).unwrap();
    assert_ne!(a, b);
}

#[test]
fn simulator_is_order_independent_and_repeatable() {
    let script = SceneScript::demo();
    let mut first = SimulatorSource::new(script.clone(), config(7)).unwrap();
    let mut second = SimulatorSource::new(script, config(7)).unwrap();
    assert_eq!(drain(&mut first), drain(&mut second));
}

// ---------------------------------------------------------------------------
// Frames and ground truth
// ---------------------------------------------------------------------------

#[test]
fn simulator_yields_every_frame_and_then_ends() {
    let script = SceneScript::demo();
    let mut source = SimulatorSource::new(script, config(3)).unwrap();
    let expected = source.frame_count();
    assert_eq!(expected, source.total_ticks() * 2);

    let frames = drain(&mut source);
    assert_eq!(frames.len(), expected);
    assert_eq!(source.recv().unwrap(), FrameOrEnd::End);

    // Every frame round-trips the wire codec and obeys the contract.
    for env in &frames {
        assert_eq!(env.frame.amplitude.len(), 64);
        assert_eq!(env.frame.phase.len(), 64);
        env.frame.validate().unwrap();
    }
}

#[test]
fn simulator_emits_synthesised_ground_truth() {
    let script = SceneScript::demo();
    let source = SimulatorSource::new(script, config(3)).unwrap();
    let truth = source.ground_truth();
    assert!(!truth.is_empty());
    assert!(truth.iter().any(|(_, r)| matches!(
        &r.payload,
        EventPayload::Presence { occupied: true, .. }
    )));
    assert!(truth.iter().any(|(_, r)| matches!(
        &r.payload,
        EventPayload::Vitals { metric, .. } if metric.as_str() == "breathing_rate"
    )));
    assert!(truth.iter().any(|(_, r)| matches!(
        &r.payload,
        EventPayload::Count { people: 1 }
    )));
}

#[test]
fn shutdown_stops_a_simulator() {
    let script = SceneScript::demo();
    let mut source = SimulatorSource::new(script, config(3)).unwrap();
    source.shutdown();
    assert_eq!(source.recv().unwrap_err().kind(), "stopped");
}

// ---------------------------------------------------------------------------
// Impairments
// ---------------------------------------------------------------------------

#[test]
fn loss_impairment_creates_sequence_gaps() {
    let mut script = SceneScript::demo();
    script.impairments.loss_prob = 0.5;
    let mut source = SimulatorSource::new(script, config(11)).unwrap();
    let expected = source.frame_count();
    let frames = drain(&mut source);
    assert!(
        frames.len() < expected,
        "loss must drop some frames: got {} of {expected}",
        frames.len()
    );

    let sequences: Vec<u64> = frames
        .iter()
        .filter(|env| env.frame.node_id == NodeId(1))
        .map(|env| env.frame.sequence)
        .collect();
    let span = sequences.last().copied().unwrap_or(0) - sequences.first().copied().unwrap_or(0) + 1;
    assert!(span as usize > sequences.len(), "expected a sequence gap");
}

#[test]
fn impairment_is_reproducible() {
    let mut script = SceneScript::demo();
    script.impairments.loss_prob = 0.3;
    script.impairments.jitter_us = 50.0;
    script.impairments.impaired_prob = 0.2;
    let a = synthesise_session(&script, &config(99)).unwrap();
    let b = synthesise_session(&script, &config(99)).unwrap();
    assert_eq!(a, b);
}

// ---------------------------------------------------------------------------
// Interlock: simulator → session → replay
// ---------------------------------------------------------------------------

#[test]
fn simulator_session_replays_to_identical_frames() {
    let script = SceneScript::demo();
    let config = config(4242);
    let bytes = synthesise_session(&script, &config).unwrap();

    // Frames produced directly by the simulator.
    let mut direct = SimulatorSource::new(script, config).unwrap();
    let direct_frames: Vec<CsiFrame> = drain(&mut direct)
        .into_iter()
        .map(|env| env.frame)
        .collect();
    assert!(!direct_frames.is_empty());

    // Frames recovered by reading the session back through replay.
    let reader = SessionReader::from_bytes(&bytes).unwrap();
    let mut replay = ReplaySource::from_reader(
        reader,
        ReplayMode::Deterministic,
        Arc::new(FixedClock(1_000)),
    );
    let replayed: Vec<CsiFrame> = drain(&mut replay).into_iter().map(|env| env.frame).collect();

    assert_eq!(
        replayed, direct_frames,
        "replay must reproduce the simulator's frames exactly"
    );

    // …and the container's digest verifies.
    let report = verify_bytes(&bytes).unwrap();
    assert_eq!(report.frames, direct_frames.len() as u64);
    assert!(report.record_count > report.frames, "ground truth included");
}
