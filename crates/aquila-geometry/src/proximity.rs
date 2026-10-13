//! Node-pair scope and the discrete proximity relation: the primary estimator.
//!
//! **Proximity fingerprinting** is the robust, primary estimator
//! (`geometry-calibration.md` §5). It makes **no metric distance claim**: it
//! yields a discrete relation per link — `same_zone_candidate | adjacent | far`
//! — which multipath can only degrade in confidence, never turn into a wrong
//! number.
//!
//! In v1 a link's relation is derived from its topology scope
//! (`data-contracts.md` §3): an intra-room link is a `same_zone_candidate`, an
//! inter-room (same-floor) link is `adjacent`, and an inter-floor link is `far`.
//! A calibrated per-zone signature replaces this prior once
//! `aquila-calibration` supplies one; the topology relation is the honest
//! fallback that needs no RF.

use aquila_types::{NodeId, PairScope};
use serde::{Deserialize, Serialize};

/// One link observation within an attribution window: the unordered node pair
/// and the whole-link received signal strength.
///
/// Only the endpoints and RSSI are needed at level 1; the per-subcarrier
/// perturbation features that a calibrated fingerprint consumes arrive with
/// `aquila-calibration`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LinkObservation {
    /// The transmitting node.
    pub tx: NodeId,
    /// The receiving node.
    pub rx: NodeId,
    /// Whole-link received signal strength, in dBm.
    pub rssi_dbm: f32,
}

impl LinkObservation {
    /// Construct a link observation.
    #[must_use]
    pub const fn new(tx: NodeId, rx: NodeId, rssi_dbm: f32) -> Self {
        Self { tx, rx, rssi_dbm }
    }
}

/// The discrete proximity relation a link reports, in the vocabulary of
/// `geometry-calibration.md` §5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProximityRelation {
    /// Both endpoints share a zone: the perturbation is likely local.
    SameZoneCandidate,
    /// The endpoints share a floor but not a room: an adjacent room.
    Adjacent,
    /// The endpoints are on different floors: far.
    Far,
}

/// The v1 topology prior mapping a pair scope to a proximity relation.
#[must_use]
pub const fn relation_for_scope(scope: PairScope) -> ProximityRelation {
    match scope {
        PairScope::Room => ProximityRelation::SameZoneCandidate,
        PairScope::SameFloor => ProximityRelation::Adjacent,
        PairScope::CrossFloor => ProximityRelation::Far,
    }
}

/// The spec-vocabulary label for a pair scope: `intra_room`, `inter_room` or
/// `inter_floor` (`geometry-calibration.md` §1).
#[must_use]
pub const fn scope_label(scope: PairScope) -> &'static str {
    match scope {
        PairScope::Room => "intra_room",
        PairScope::SameFloor => "inter_room",
        PairScope::CrossFloor => "inter_floor",
    }
}
