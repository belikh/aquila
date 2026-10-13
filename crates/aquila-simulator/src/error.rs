//! The typed error surface for the deterministic simulator.
//!
//! Simulation is a pure function and never panics: an invalid scene, a session
//! write failure or a wire round-trip failure is a typed [`SimError`].

use thiserror::Error;

/// Everything that can go wrong building or running the simulator.
#[derive(Debug, Error)]
pub enum SimError {
    /// The scene script or simulation configuration is invalid.
    #[error("invalid simulation configuration: {0}")]
    Config(String),
    /// A session container could not be written.
    #[error("session error: {0}")]
    Session(#[from] aquila_session::SessionError),
    /// The `CsiSource` seam reported an error.
    #[error("source error: {0}")]
    Source(#[from] aquila_sources::SourceError),
    /// A ground-truth record could not be serialised.
    #[error("ground-truth JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl SimError {
    /// A short, stable label for the failure, useful in tests and logs.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Config(_) => "config",
            Self::Session(_) => "session",
            Self::Source(_) => "source",
            Self::Json(_) => "json",
        }
    }
}
