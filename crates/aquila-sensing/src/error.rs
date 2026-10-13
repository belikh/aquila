//! The typed error surface of the sensing core.
//!
//! Nothing in the core panics on malformed input: every failure is a typed
//! [`SensingError`], and a refused output sink is a typed [`SinkError`]. A
//! caller can therefore act on the failure without re-deriving it, and a
//! malformed frame is rejected and counted rather than aborting the pipeline.
//!
//! The errors are implemented by hand rather than with a derive macro so the
//! crate keeps to its declared dependency set.

use std::fmt;

use aquila_geometry::GeometryError;
use aquila_signal::SignalError;
use aquila_sources::SourceError;
use aquila_types::ValidationError;

/// A failure of the output [`Sink`](crate::Sink) seam.
///
/// A sink is the one place the core writes its decisions; a refusal is surfaced
/// rather than swallowed, so a broken stream is visible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinkError {
    /// The sink has been closed and can no longer accept events.
    Closed,
    /// Any other sink failure, carrying a human-readable reason.
    Other(String),
}

impl fmt::Display for SinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => f.write_str("the sink has been closed"),
            Self::Other(reason) => write!(f, "sink error: {reason}"),
        }
    }
}

impl std::error::Error for SinkError {}

/// Everything that can go wrong stepping the sensing core.
#[derive(Debug)]
pub enum SensingError {
    /// The CSI source failed on a pull.
    Source(SourceError),
    /// The signal-conditioning stage rejected a window.
    Signal(SignalError),
    /// A contract value failed its invariant check.
    Validation(ValidationError),
    /// The geometry module rejected a topology query or load.
    Geometry(GeometryError),
    /// The metric sink refused a record.
    Metrics(aquila_metrics::MetricsError),
    /// The output sink refused an event.
    Sink(SinkError),
    /// The configuration is internally inconsistent.
    Config(String),
    /// A structural invariant of the core was broken.
    Invariant(String),
}

impl fmt::Display for SensingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(e) => write!(f, "source error: {e}"),
            Self::Signal(e) => write!(f, "signal error: {e}"),
            Self::Validation(e) => write!(f, "contract validation error: {e}"),
            Self::Geometry(e) => write!(f, "geometry error: {e}"),
            Self::Metrics(e) => write!(f, "metric sink error: {e}"),
            Self::Sink(e) => write!(f, "sink error: {e}"),
            Self::Config(reason) => write!(f, "invalid sensing config: {reason}"),
            Self::Invariant(reason) => write!(f, "sensing invariant violated: {reason}"),
        }
    }
}

impl std::error::Error for SensingError {}

impl From<SourceError> for SensingError {
    fn from(error: SourceError) -> Self {
        Self::Source(error)
    }
}

impl From<SignalError> for SensingError {
    fn from(error: SignalError) -> Self {
        Self::Signal(error)
    }
}

impl From<ValidationError> for SensingError {
    fn from(error: ValidationError) -> Self {
        Self::Validation(error)
    }
}

impl From<GeometryError> for SensingError {
    fn from(error: GeometryError) -> Self {
        Self::Geometry(error)
    }
}

impl From<aquila_metrics::MetricsError> for SensingError {
    fn from(error: aquila_metrics::MetricsError) -> Self {
        Self::Metrics(error)
    }
}

impl From<SinkError> for SensingError {
    fn from(error: SinkError) -> Self {
        Self::Sink(error)
    }
}
