//! The typed error surface of `aquila-capture`.
//!
//! Every failure is a typed [`CaptureError`]; the binary never panics and never
//! unwraps on user input.

use thiserror::Error;

/// Everything that can go wrong running `aquila-capture`.
#[derive(Debug, Error)]
pub(crate) enum CaptureError {
    /// The command line was malformed.
    #[error("usage: {0}")]
    Usage(String),
    /// A file or socket operation failed.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    /// A `.aqs` container could not be read, written or verified.
    #[error("session error: {0}")]
    Session(#[from] aquila_session::SessionError),
    /// The deterministic simulator refused the scene.
    #[error("simulator error: {0}")]
    Simulator(#[from] aquila_simulator::SimError),
    /// The live source failed.
    #[error("source error: {0}")]
    Source(#[from] aquila_sources::SourceError),
    /// A provisioning document could not be serialised.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// Any other failure, carrying a human-readable reason.
    #[error("{0}")]
    Other(String),
}
