#![forbid(unsafe_code)]

//! `aquila-sources` — the core↔node seam.
//!
//! This crate is level 3 in the workspace dependency graph
//! (`docs/spec/interface-register.md`) and depends only on [`aquila_types`],
//! [`aquila_wire`] and [`aquila_session`]. It owns the one seam where "where CSI
//! comes from" is decided ([`CsiSource`]), its adapters, and the outbound half
//! of the same protocol ([`NodeControl`]) — not a seventh seam (`ADR-0001` §3).
//!
//! Adapters:
//!
//! - [`ReplaySource`] — reads a Session and replays it through the real
//!   `aquila-wire` decoder. Deterministic mode re-stamps arrivals from an
//!   injected [`Clock`](aquila_types::Clock) for byte-identical CI replay.
//! - [`UdpLiveSource`] — one UDP socket on the CSI data port (9100), all nodes,
//!   tolerating and counting malformed datagrams.
//! - [`FanInSource`] — multiplexes several sources into one ordered stream.
//! - `SimulatorSource` lives in `aquila-simulator` and implements the same
//!   trait, so replay, live and simulation are interchangeable.
//!
//! The identity seam [`NodeIdentityMap`] (`ADR-0003` §3) is re-exported from
//! `aquila-wire` so a consumer of this crate need not depend on the codec crate
//! directly to translate between factory MACs and dense node identities.
//!
//! Australian English throughout (serialisation, behaviour, normalise).

mod control;
mod error;
mod fanin;
mod live;
mod replay;
mod seam;

#[cfg(test)]
mod tests;

pub use control::NodeControl;
pub use error::SourceError;
pub use fanin::FanInSource;
pub use live::{LiveConfig, LiveStats, LiveStatsSnapshot, UdpLiveSource};
pub use replay::{ReplayMode, ReplaySource};
pub use seam::{CsiSource, FrameOrEnd};

pub use aquila_wire::{NodeIdentityMap, CONTROL_PORT, CSI_DATA_PORT};
