//! The error type for the measurement module.
//!
//! Everything that can fail in `aquila-metrics` — reading or writing the
//! ledger, serialising a record, validating a malformed measurement, and a
//! required sink in a [`TeeSink`](crate::TeeSink) refusing a record — surfaces
//! as a [`MetricsError`]. The error carries no policy: in v1 a record error is
//! reported, never used to gate a build (`measurement.md` §5).

use std::io;

/// The error returned by validation, serialisation and the sink seam.
#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    /// The ledger could not be read or written.
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
    /// A record could not be canonically serialised or parsed.
    #[error("serialisation error: {0}")]
    Serde(#[from] serde_json::Error),
    /// A [`Measurement`](crate::Measurement) broke one of its invariants.
    #[error("invalid measurement: {0}")]
    Invalid(String),
    /// A required sink in a tee refused the record; the first such failure is
    /// returned after every sink has been tried.
    #[error("a required metric sink failed: {0}")]
    RequiredSink(String),
}
