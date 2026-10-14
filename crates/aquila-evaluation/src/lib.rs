#![forbid(unsafe_code)]

//! `aquila-evaluation` — the `GroundTruth` seam, the replay/simulation harness
//! and scoring (level 6, `docs/spec/interface-register.md`).
//!
//! This crate owns the **ground-truth seam** and the offline scorer of
//! `docs/spec/evaluation.md`:
//!
//! - [`GroundTruth`] — the only source of reference labels; never a runtime
//!   input. [`FixtureGroundTruth`] is the v1 default adapter, labelling a
//!   Session from a committed fixture set or a scene's synthesised reference
//!   stream.
//! - [`Harness::run`] — pairs a run's emitted events with reference records on
//!   the common clock and returns an [`EvaluationReport`], refusing to state a
//!   metric it cannot pair (zero paired samples is *not measured*).
//! - [`CollectorSink`] — the evaluation adapter for the `Sink` seam
//!   (`interface-register.md` "Seams and their adapters").
//!
//! # Prose
//!
//! Australian English throughout (normalise, behaviour, serialisation).

mod collector;
mod error;
mod ground_truth;
mod harness;

#[cfg(test)]
mod tests;

pub use collector::CollectorSink;
pub use error::EvaluationError;
pub use ground_truth::{
    Coverage, EvalScope, FixtureGroundTruth, GroundTruth, ReferenceKind, ReferenceProvenance,
    ReferenceRecord,
};
pub use harness::{presence_f1, EvaluationReport, Harness};
