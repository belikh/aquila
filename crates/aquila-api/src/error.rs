//! The typed error surface of `aquila-api`.

use std::net::SocketAddr;

/// Everything that can go wrong starting or serving the control plane.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// The requested bind address is not loopback. The control plane is
    /// loopback-only (`core-api.md` §2, §15).
    #[error("address {0} is not loopback; the control plane binds loopback only")]
    NonLoopback(SocketAddr),
    /// A socket or thread error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// A malformed HTTP request.
    #[error("http error: {0}")]
    Http(String),
    /// The WebSocket upgrade is a deliberately unimplemented, fail-closed stub.
    ///
    /// TODO(core-api): implement the RFC 6455 upgrade and `aquila.v1`
    /// subscription stream (`core-api.md` §10). Until then this is typed and
    /// refuses rather than silently accepting an upgrade it cannot serve.
    #[error("websocket upgrade is not implemented")]
    WebSocketUnimplemented,
    /// Any other failure, carrying a human-readable reason.
    #[error("{0}")]
    Other(String),
}
