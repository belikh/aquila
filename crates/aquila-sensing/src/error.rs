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

/// The reason a stored calibration artefact may no longer be used.
///
/// Invalidation is *explicit*, never silent (`geometry-calibration.md` §10,
/// ticket acceptance criterion "Global-by-default gate"): a mismatch is a
/// typed reason a caller can surface, and a stale artefact is refused rather
/// than reused.
#[derive(Debug, Clone, PartialEq)]
pub enum InvalidationReason {
    /// The artefact's schema major is not the one this core understands.
    SchemaMismatch {
        /// The schema version recorded in the artefact.
        found: String,
        /// The schema version this core accepts.
        expected: String,
    },
    /// The learning-semantics version differs, so thresholds are not comparable.
    SemanticsMismatch {
        /// The semantics version recorded in the artefact.
        found: String,
        /// The semantics version this core produced.
        expected: String,
    },
    /// The core commit that produced the artefact differs from the current one.
    CoreCommitChanged {
        /// The commit recorded in the artefact.
        found: String,
        /// The current commit.
        expected: String,
    },
    /// The topology changed since the artefact was captured.
    TopologyChanged {
        /// The topology digest recorded in the artefact.
        found: String,
        /// The current topology digest.
        expected: String,
    },
    /// A node's firmware changed since the artefact was captured.
    FirmwareChanged {
        /// The firmware recorded in the artefact.
        found: String,
        /// The current firmware.
        expected: String,
    },
    /// A model the calibration assumed changed.
    ModelChanged {
        /// The model versions recorded in the artefact.
        found: Vec<String>,
        /// The current model versions.
        expected: Vec<String>,
    },
    /// A node drifted beyond the calibrated residual budget.
    DriftExceeded {
        /// The node that drifted.
        node: u16,
        /// The residual observed at calibration, in milliseconds.
        calibrated_ms: f64,
        /// The live residual, in milliseconds.
        observed_ms: f64,
    },
    /// The baseline is older than the policy's staleness window.
    BaselineAgeExceeded {
        /// The age of the baseline, in days.
        age_days: u32,
        /// The policy's staleness window, in days.
        stale_after_days: u32,
    },
    /// A quality gate failed, so the artefact is not `valid`.
    QualityFailed,
}

impl fmt::Display for InvalidationReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch { found, expected } => {
                write!(f, "schema {found} is not the accepted {expected}")
            }
            Self::SemanticsMismatch { found, expected } => {
                write!(f, "calibration semantics {found} != {expected}")
            }
            Self::CoreCommitChanged { found, expected } => {
                write!(f, "core commit {found} != {expected}")
            }
            Self::TopologyChanged { found, expected } => {
                write!(f, "topology digest {found} != {expected}")
            }
            Self::FirmwareChanged { found, expected } => {
                write!(f, "firmware {found} != {expected}")
            }
            Self::ModelChanged { found, expected } => {
                write!(f, "model versions {found:?} != {expected:?}")
            }
            Self::DriftExceeded {
                node,
                calibrated_ms,
                observed_ms,
            } => write!(
                f,
                "node {node} drift residual {observed_ms} ms exceeds calibrated {calibrated_ms} ms"
            ),
            Self::BaselineAgeExceeded {
                age_days,
                stale_after_days,
            } => write!(
                f,
                "baseline age {age_days} d exceeds the {stale_after_days} d staleness window"
            ),
            Self::QualityFailed => f.write_str("a calibration quality gate failed"),
        }
    }
}

/// A failure of calibration, its artefact or its store.
///
/// Every variant is typed so a caller can act on the failure — re-run, prompt
/// the user, or refuse to sense on a stale artefact — without re-deriving it.
#[derive(Debug, Clone, PartialEq)]
pub enum CalibrationError {
    /// The calibration store could not read or write.
    Io(String),
    /// The artefact's canonical JSON was malformed.
    Json(String),
    /// The artefact's recomputed digest did not match its stored digest.
    DigestMismatch {
        /// The digest recorded in the artefact.
        expected: String,
        /// The digest recomputed from the artefact's body.
        actual: String,
    },
    /// The artefact's schema major is unknown to this core.
    UnsupportedSchema {
        /// The schema version found.
        found: String,
    },
    /// The artefact is no longer valid and must not be reused silently.
    Stale(Vec<InvalidationReason>),
    /// No artefact with the requested identity exists.
    NotFound {
        /// The missing calibration identity.
        calibration_id: String,
    },
    /// A calibration input was structurally invalid (for example empty).
    Invalid(String),
}

impl fmt::Display for CalibrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(reason) => write!(f, "calibration store I/O error: {reason}"),
            Self::Json(reason) => write!(f, "calibration JSON error: {reason}"),
            Self::DigestMismatch { expected, actual } => {
                write!(f, "calibration digest mismatch: {expected} != {actual}")
            }
            Self::UnsupportedSchema { found } => {
                write!(f, "unsupported calibration schema {found}")
            }
            Self::Stale(reasons) => {
                let joined = reasons
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(f, "calibration is stale: {joined}")
            }
            Self::NotFound { calibration_id } => {
                write!(f, "no calibration artefact {calibration_id}")
            }
            Self::Invalid(reason) => write!(f, "invalid calibration input: {reason}"),
        }
    }
}

impl std::error::Error for CalibrationError {}

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
    /// A calibration command, artefact or store failed.
    Calibration(CalibrationError),
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
            Self::Calibration(e) => write!(f, "calibration error: {e}"),
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

impl From<CalibrationError> for SensingError {
    fn from(error: CalibrationError) -> Self {
        Self::Calibration(error)
    }
}
