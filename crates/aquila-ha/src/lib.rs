#![forbid(unsafe_code)]

//! `aquila-ha` — the Home Assistant-facing output adapter (level 6,
//! `docs/spec/interface-register.md`).
//!
//! [`HaSink`] implements the `Sink` seam owned by `aquila-sensing` and projects
//! each edge-triggered [`Event`](aquila_types::Event) into an HA-shaped
//! [`EntityModel`]: a home → floor → room → node tree of entity states with
//! Home Assistant's naming, units and device classes (`home-assistant.md` §4,
//! §6). It transports nothing — `aquila-api` owns the control plane and reads
//! [`EntityModel::snapshot`] to serve `/api/v1/state`.
//!
//! It carries no Home Assistant dependency: the projection is a plain in-memory
//! model (a map of entity id → state) plus a JSON snapshot of the same shape the
//! Python custom component consumes. This keeps the mapping testable without an
//! HA host (`repository-tooling.md` §3.2 pattern).
//!
//! # Scope
//!
//! v1 projects presence per zone, breathing and heart rate per zone, and home
//! occupancy and person count. Floor-scoped aggregation is reported in the
//! snapshot but has no dedicated entity yet (`home-assistant.md` §6.1). A
//! `null` (unknown) vital maps to HA `unknown`, never a fabricated zero.
//!
//! # Prose
//!
//! Australian English throughout (normalise, behaviour, serialisation).

mod entity;
mod sink;

#[cfg(test)]
mod tests;

pub use entity::{EntityKind, EntityModel, EntityState, ZoneState};
pub use sink::HaSink;
