//! The typed error surface for the node↔core seam.
//!
//! Pulling a frame never panics: a socket failure, a malformed datagram, a
//! session error or a pending pull after shutdown is a typed [`SourceError`].
//! The live adapter counts malformed datagrams and continues rather than ending
//! the stream (`capture-and-firmware.md` §3.2); only genuine I/O failures end it.

use thiserror::Error;

/// Everything that can go wrong pulling a frame from a [`CsiSource`].
///
/// [`CsiSource`]: crate::CsiSource
#[derive(Debug, Error)]
pub enum SourceError {
    /// An underlying socket or file operation failed.
    #[error("source I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// An on-wire datagram could not be decoded.
    #[error("wire decode error: {0}")]
    Decode(aquila_wire::DecodeError),
    /// A session container could not be read.
    #[error("session error: {0}")]
    Session(#[from] aquila_session::SessionError),
    /// The source has been shut down; a pending or subsequent `recv` is stopped.
    #[error("source has been shut down")]
    Stopped,
    /// A blocking control exchange exceeded its deadline without a matching
    /// reply.
    #[error("control {operation} timed out")]
    Timeout {
        /// The operation that timed out, for example `"request"`.
        operation: &'static str,
    },
    /// Any other source failure, carrying a human-readable reason.
    #[error("source error: {0}")]
    Other(String),
}

impl From<aquila_wire::DecodeError> for SourceError {
    fn from(error: aquila_wire::DecodeError) -> Self {
        Self::Decode(error)
    }
}

impl SourceError {
    /// A short, stable label for the failure, useful in tests and logs.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Decode(_) => "decode",
            Self::Session(_) => "session",
            Self::Stopped => "stopped",
            Self::Timeout { .. } => "timeout",
            Self::Other(_) => "other",
        }
    }
}
