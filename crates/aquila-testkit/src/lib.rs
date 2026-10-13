#![forbid(unsafe_code)]

//! `aquila-testkit` — dev-only fakes, fixtures and a deterministic generator.
//!
//! This crate is first-class, in-tree product infrastructure, not agent
//! scaffolding (`testing.md` §3). It provides the fake adapters for the six
//! seams, a deterministic scene generator and fixture loader, and the scoring
//! helpers the tracer and evaluation tiers use. Every fake implements the same
//! trait as the production adapter, so a fake cannot silently drift from the
//! seam contract.
//!
//! # Prose
//!
//! Australian English throughout (serialisation, behaviour, normalise).

mod clock;
mod fixture;
mod generator;
mod metrics;
mod models;
mod scoring;
mod sink;

#[cfg(test)]
mod tests;

pub use clock::FakeClock;
pub use fixture::{Fixture, FixtureError};
pub use generator::{topology_for, DeterministicCsiGenerator};
pub use metrics::RecordingMetricSink;
pub use models::{FakePoseModel, FakeVitalsModel};
pub use scoring::{count_mae, presence_f1, window_index, PresenceScore};
pub use sink::CollectorSink;
