#![forbid(unsafe_code)]

//! `aquila-metrics` — the metric definitions and the commit-keyed ledger behind
//! the `MetricSink` seam (`docs/spec/measurement.md`).
//!
//! Aquila treats measurement as a first-class module. Every claim it makes about
//! itself — correctness, determinism, structure, performance, accuracy — is a
//! [`Measurement`] written to a commit-keyed ledger through a single write-only
//! seam ([`MetricSink`]) and compared run-to-run by [`diff`]. In v1 the ledger
//! **records and diffs only; nothing hard-gates CI** (`measurement.md` §5).
//!
//! This crate sits at level 1 of the dependency DAG: it depends on the
//! vocabulary crate `aquila-types` and the pre-provisioned serialisation
//! dependencies, and nothing in the DSP, inference, fusion or calibration graph
//! depends on it.
//!
//! # Identity
//!
//! A record's identity is the SHA-256 of its canonical serialisation **with
//! `timestamp` removed**, so re-recording a measurement does not mint a new
//! identity and the timestamp never participates in comparisons. The identity is
//! derived ([`Measurement::id`]), never stored.
//!
//! # Honesty guards
//!
//! A dirty `head`, a host mismatch on a host-sensitive metric, or a change of
//! `version`, `unit` or `grade` downgrades a comparison: it is labelled on the
//! [`Delta`], never hidden.
//!
//! # Prose
//!
//! Australian English throughout (serialisation, behaviour, normalise).

mod diff;
mod error;
mod measurement;
mod metric;
mod sha256;
mod sink;

#[cfg(test)]
mod tests;

pub use diff::{
    diff, diff_with, Bar, Delta, DiffOptions, Direction, Downgrade, Verdict, DEFAULT_TOLERANCE_PCT,
};
pub use error::MetricsError;
pub use measurement::{
    Authority, CommitSha, Grade, HostFingerprint, Measurement, MeasurementId, MeasurementValue,
    MetricName, Timestamp, ToolVersions, Unit,
};
pub use metric::{record_metric, Metric};
pub use sink::{InMemoryMetricSink, JsonLedgerSink, MetricSink, TeeSink};
