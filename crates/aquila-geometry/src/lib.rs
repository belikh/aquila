#![forbid(unsafe_code)]
//! `aquila-geometry` — home topology, coordinate frames and confidence-scored
//! spatial attribution (level 1, `docs/spec/interface-register.md`).
//!
//! # Scope
//!
//! Geometry answers *where things are*, and how confidently. It owns:
//!
//! - the indexed topology over the contract model ([`Topology`], built from
//!   `aquila_types::Topology` by [`Topology::load`]);
//! - attribution of a node to room, zone and floor
//!   ([`Topology::room_of`], [`Topology::zone_of`], [`Topology::floor_of`]) and
//!   its surveyed position ([`Topology::position`]);
//! - node-pair scope and the canonical unordered pair identity
//!   ([`Topology::scope_of`], [`Topology::pairs`]);
//! - confidence-scored planar frames ([`Transform2`], [`Frame`]);
//! - proximity fingerprinting as the primary estimator and bounded RSSI
//!   trilateration as an **advisory** secondary estimator
//!   ([`Topology::estimate_position`]).
//!
//! It is pure computation: no I/O, no async, no model crates. It depends only on
//! `aquila-types` (`geometry-calibration.md` §6).
//!
//! # The hard limit
//!
//! Each node has its own oscillator and sampling clock, and there is no
//! cross-node phase coherence, so there is **no cross-node angle-of-arrival, no
//! ambiguity-free time-of-flight and no coherent array gain**
//! (`geometry-calibration.md` §0). Indoor RSSI is multipath-biased and a
//! path-loss range is accurate to roughly a factor of two at best. Geometry is
//! therefore **approximate and confidence-scored**: room-level is guaranteed
//! given a usable baseline, zone-level is best-effort, and any continuous
//! coordinate is **advisory only** and never a hard input to presence, pose or
//! vitals.
//!
//! # Coordinate-frame decision
//!
//! There is exactly one physical frame, the **home frame** of
//! `data-contracts.md` §3.1: Cartesian, metres, right-handed, `x` east, `y`
//! north, `z` up, origin at the home's declared reference. Floors, rooms and
//! nodes are not independent world frames; each is reached from the home frame
//! by a confidence-scored planar SE(2) transform ([`Transform2`]) composed from
//! the model's own fields — a floor by its `elevation_m`, a room by
//! `origin_home_m` and `yaw_deg`, a node by its room-local `pos_room_m`.
//! Composition multiplies confidence and an unsurveyed placement is the
//! identity with zero confidence, never a guess. The non-physical `CsiBand`
//! subcarrier space is deliberately not a frame.
//!
//! # Naming
//!
//! [`Topology`] here is geometry's indexed wrapper; the contract record of the
//! same name is [`aquila_types::Topology`] and is re-exported as the model every
//! shape lives in.

mod confidence;
mod error;
mod frame;
mod position;
mod proximity;
mod topology;

#[cfg(test)]
mod tests;

pub use confidence::{chain, conjunction, independent, unit_confidence, Scored};
pub use error::GeometryError;
pub use frame::{Frame, Transform2, FRAME_CONFIDENCE_CONFIGURED};
pub use position::{
    link_confidence, range_sigma_m, trilaterate, ErrorEllipse, PathLossModel, PositionEstimate,
    PositionMethod, PositionResolution, Trilateration, AMBIGUITY_MARGIN, CHI2_95_2DOF,
    DEFAULT_PATH_LOSS_EXPONENT, DEFAULT_REFERENCE_DISTANCE_M, DEFAULT_REFERENCE_RSSI_DBM,
    DEFAULT_ROOM_EXTENT_M, LINK_RSSI_FLOOR_DBM, LINK_RSSI_SPAN_DBM, MAX_RANGE_M, MIN_RANGE_M,
    MIN_RANGE_SIGMA_M, RANGE_SIGMA_FRACTION, TRILATERATION_CONFIDENCE_CAP,
};
pub use proximity::{relation_for_scope, scope_label, LinkObservation, ProximityRelation};
pub use topology::Topology;

// The contract vocabulary geometry operates on, re-exported so a caller needs
// one crate. `aquila-types` remains the single owner of every shape.
pub use aquila_types::{
    Confidence, DeviceType, FirmwareRef, Floor, FloorId, Home, MacAddr, Node, NodeId, NodePair,
    PairScope, Room, RoomId, RoomKind, ValidationError, ZoneId,
};
