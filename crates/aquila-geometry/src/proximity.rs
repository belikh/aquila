//! Node-pair proximity and the discrete proximity relation: the primary
//! estimator.
//!
//! **Proximity fingerprinting** is the robust, primary estimator
//! (`geometry-calibration.md` §5). It makes **no metric distance claim**: it
//! yields a discrete relation per link — `same_zone_candidate | adjacent | far`
//! — which multipath can only degrade in confidence, never turn into a wrong
//! number.
//!
//! A link's relation is derived from the 3D distance between its endpoints'
//! surveyed positions: co-located endpoints are a `same_zone_candidate`,
//! moderately separated endpoints are `adjacent`, and distant endpoints are
//! `far`. A calibrated per-zone signature replaces this prior once
//! `aquila-calibration` supplies one; the proximity relation is the honest
//! fallback that needs no RF.

use aquila_types::NodeId;
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
    /// Endpoints are co-located or close: the perturbation is likely local.
    SameZoneCandidate,
    /// Endpoints are moderately separated: an adjacent region.
    Adjacent,
    /// Endpoints are distant: far.
    Far,
}

/// Estimate the proximity relation from the 3D distance between two nodes'
/// surveyed positions.
#[must_use]
pub fn relation_for_distance(distance_m: f32) -> ProximityRelation {
    if distance_m <= 2.0 {
        ProximityRelation::SameZoneCandidate
    } else if distance_m <= 6.0 {
        ProximityRelation::Adjacent
    } else {
        ProximityRelation::Far
    }
}
