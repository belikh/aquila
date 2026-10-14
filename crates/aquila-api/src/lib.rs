#![forbid(unsafe_code)]

//! `aquila-api` — the private loopback HTTP control plane (level 7,
//! `docs/spec/interface-register.md`).
//!
//! It is **not** a public interface: no REST/WS/MCP is offered to third parties
//! (`core-api.md` §1). It exposes:
//!
//! - [`Server::bind`] / [`Server::handle`] — start a loopback HTTP/1.1 server
//!   and obtain a [`Handle`];
//! - [`Handle::publish`] — fold an event into the HA-shaped entity model and
//!   fan it out to subscribers;
//! - [`ChannelSink`] — the `Sink` adapter that connects the sensing core to a
//!   running server (`interface-register.md` "Seams and their adapters").
//!
//! It serves `GET /api/v1/live`, `GET /api/v1/health` and `GET /api/v1/state`
//! from the shared snapshot, and refuses WebSocket upgrades with the typed,
//! fail-closed [`ApiError::WebSocketUnimplemented`] until the RFC 6455 stream is
//! implemented (`core-api.md` §10). The listener binds loopback only.
//!
//! # Prose
//!
//! Australian English throughout (serialisation, behaviour, normalise).

mod error;
mod handle;
mod http;
mod server;

#[cfg(test)]
mod tests;

pub use error::ApiError;
pub use handle::{ChannelSink, Handle};
pub use server::{Server, ServerGuard};

// The outbound half of the core↔node protocol lives in `aquila-sources`; it is
// re-exported here so the composition root can reach it through the control
// plane (the node-command relay this crate will own, `core-api.md` §9).
pub use aquila_sources::NodeControl;
