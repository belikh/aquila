#![forbid(unsafe_code)]

//! `aquila-api` — the private loopback HTTP control plane (level 7,
//! `docs/spec/interface-register.md`).
//!
//! It is **not** a public interface: no REST/WS/MCP is offered to third parties
//! (`core-api.md` §1). It exposes:
//!
//! - [`Server::bind`] / [`Server::bind_with_auth`] — start a loopback HTTP/1.1
//!   server and obtain a [`Handle`];
//! - [`Handle::publish`] — fold an event into the HA-shaped entity model and
//!   fan it out to subscribers;
//! - [`ChannelSink`] — the `Sink` adapter that connects the sensing core to a
//!   running server (`interface-register.md` "Seams and their adapters");
//! - [`Auth`] — the loopback/ingress trust policy (`core-api.md` §3).
//!
//! # Canonical routes (`core-api.md` §6)
//!
//! Served from the shared snapshot and the topology-derived projections:
//! `GET /api/v1/{live,health,state,schema,capabilities,nodes,firmware}`,
//! `GET /api/v1/nodes/{node}/firmware`, `GET /api/v1/nodes/{node}/firmware/job`,
//! and `GET /api/v1/metrics/snapshot`. The OTA routes are backed by the
//! [`Ota`] runtime: `POST .../firmware/install` begins a job and
//! `POST .../firmware/abort` aborts it. On a core with no signed image store or
//! node transport wired, the mutating routes fail closed with a typed
//! `503 ota_unavailable` (ticket #13 implements the core side; the node
//! firmware agent is #14).
//!
//! # Transport decision
//!
//! v1 is **HTTP polling only**. `GET /api/v1/stream` returns a typed
//! `501 websocket_unimplemented`; the HACS component polls `GET /api/v1/state`
//! (`core-api.md` §10, `home-assistant.md` §5.2). [`Server::bind`] refuses a
//! non-loopback address; [`Server::bind_with_config`] with
//! [`ServerConfig::any_interface`] is the add-on path, where the container
//! address must be reachable through Home Assistant ingress.
//!
//! [`ServerConfig::assets`] additionally serves the WASM dashboard bundle from
//! `AQUILA_DASHBOARD_DIR` on the same listener, stripping the ingress path
//! prefix (`X-Ingress-Path`) and injecting it as the document `<base href>`
//! (`dashboard.md` §2, §4).
//!
//! # Prose
//!
//! Australian English throughout (serialisation, behaviour, normalise).

mod assets;
mod error;
mod handle;
mod http;
mod ota;
mod projection;
mod server;

#[cfg(test)]
mod tests;

pub use assets::{StaticAssets, DASHBOARD_DIR_ENV};
pub use error::ApiError;
pub use handle::{ChannelSink, Handle};
pub use ota::{
    FirmwareCompat, FirmwareJob, FirmwareJobState, FirmwareManifest, FirmwareStore, ImportRequest,
    InstallError, Ota, OtaConfig, OtaError, OtaManager, OtaStats, OtaTransport, StoreError,
    StoredImage, DEFAULT_FIRMWARE_DIR,
};
pub use server::{Auth, Server, ServerConfig, ServerGuard};

// The outbound half of the core↔node protocol lives in `aquila-sources`; it is
// re-exported here so the composition root can reach it through the control
// plane (the node-command relay this crate will own, `core-api.md` §9).
pub use aquila_sources::NodeControl;
