//! The indexed home topology: the deep module every geometry query goes
//! through.
//!
//! [`Topology`] wraps the contract `aquila_types::Topology`
//! (`data-contracts.md` §3) with precomputed lookup indices, validates the
//! model once at [`Topology::load`], and answers the register's queries:
//! attribution ([`Topology::zone_of`], [`Topology::position`]), and advisory
//! position ([`Topology::estimate_position`]).
//!
//! **Home → Zones → Nodes → NodePairs.** There are no floors and no rooms;
//! zones are the only spatial concept.
//!
//! It performs no I/O: JSON decoding stays with the caller, so geometry remains
//! pure computation (`geometry-calibration.md` §6).

use std::cmp::Ordering;
use std::collections::HashMap;

use aquila_types::{
    Confidence, Home, Node, NodeId, NodePair, Zone, ZoneId,
};

use crate::confidence::{independent, unit_confidence, Scored};
use crate::error::GeometryError;
use crate::position::{
    link_confidence, trilaterate, ErrorEllipse, PathLossModel, PositionEstimate, PositionMethod,
    PositionResolution, AMBIGUITY_MARGIN, DEFAULT_ZONE_EXTENT_M, TRILATERATION_CONFIDENCE_CAP,
};
use crate::proximity::LinkObservation;

/// An indexed, validated home topology.
///
/// Built from the contract model by [`Topology::load`]. Queries are total over
/// nodes that belong to the topology and honest (`Scored::unknown`) for
/// anything else.
#[derive(Debug, Clone)]
pub struct Topology {
    model: aquila_types::Topology,
    zone_index: HashMap<ZoneId, usize>,
    node_index: HashMap<NodeId, usize>,
    node_position: HashMap<NodeId, [f32; 3]>,
}

impl Topology {
    /// Load and index a topology, rejecting one that fails the
    /// referential-integrity rules of `data-contracts.md` §3.
    ///
    /// This is the pure, IO-free half of loading `config/topology.json`: a
    /// caller decodes the JSON with its own serde format crate and hands the
    /// model here.
    pub fn load(model: aquila_types::Topology) -> Result<Self, GeometryError> {
        model.validate()?;

        let mut zone_index = HashMap::with_capacity(model.zones.len());
        for (i, zone) in model.zones.iter().enumerate() {
            zone_index.insert(zone.id.clone(), i);
        }
        let mut node_index = HashMap::with_capacity(model.nodes.len());
        let mut node_position = HashMap::with_capacity(model.nodes.len());
        for (i, node) in model.nodes.iter().enumerate() {
            node_index.insert(node.id, i);
            node_position.insert(node.id, node.pos_home_m);
        }

        Ok(Self {
            model,
            zone_index,
            node_index,
            node_position,
        })
    }

    /// The underlying contract model.
    #[must_use]
    pub fn model(&self) -> &aquila_types::Topology {
        &self.model
    }

    /// The home.
    #[must_use]
    pub fn home(&self) -> &Home {
        &self.model.home
    }

    /// Look up a node.
    #[must_use]
    pub fn node(&self, node: NodeId) -> Option<&Node> {
        self.node_index.get(&node).map(|&i| &self.model.nodes[i])
    }

    /// Look up a zone.
    #[must_use]
    pub fn zone(&self, zone: &ZoneId) -> Option<&Zone> {
        self.zone_index.get(zone).map(|&i| &self.model.zones[i])
    }

    /// The ordered node-pairs of the sensing graph, as authored.
    #[must_use]
    pub fn pairs(&self) -> &[NodePair] {
        &self.model.pairs
    }

    /// The surveyed home-frame position of a node.
    ///
    /// A known node always has a position (`pos_home_m` is required in the
    /// contract), so this yields [`Confidence::ONE`]. An unknown node yields
    /// [`Scored::unknown`]. The value is the stored survey, not an inference;
    /// its physical accuracy is not modelled here.
    #[must_use]
    pub fn position(&self, node: NodeId) -> Scored<[f32; 3]> {
        match self.node_position.get(&node) {
            Some(p) => Scored::certain(*p),
            None => Scored::unknown(),
        }
    }

    /// Attribute a node to the zone containing its position, best-effort.
    ///
    /// A node is mounted at a home-frame position; zones are detection regions
    /// in the same space. Returns the zone whose bounding box contains the
    /// node's position; otherwise `None`, since a node between zones belongs to
    /// none (`geometry-calibration.md` §4).
    #[must_use]
    pub fn zone_of(&self, node: NodeId) -> Scored<Option<&ZoneId>> {
        let Some(p) = self.node_position.get(&node) else {
            return Scored::unknown();
        };
        let zone = self
            .model
            .zones
            .iter()
            .find(|z| {
                (0..3).all(|i| (p[i] - z.origin_home_m[i]).abs() <= z.extent_home_m[i])
            })
            .map(|z| &z.id);
        match zone {
            Some(z) => Scored::certain(Some(z)),
            None => Scored::certain(None),
        }
    }

    /// The canonical, orientation-free identity of an unordered node pair.
    ///
    /// `"a-b"` with the endpoints in ascending order, so `canonical_pair_id(a, b)
    /// == canonical_pair_id(b, a)` (`geometry-calibration.md` §1).
    #[must_use]
    pub fn canonical_pair_id(a: NodeId, b: NodeId) -> String {
        let (lo, hi) = Self::canonical_pair_key(a, b);
        format!("{}-{}", lo.0, hi.0)
    }

    /// The endpoints of an unordered node pair in ascending order.
    #[must_use]
    pub fn canonical_pair_key(a: NodeId, b: NodeId) -> (NodeId, NodeId) {
        if a <= b {
            (a, b)
        } else {
            (b, a)
        }
    }

    /// Estimate a position from a window of link observations, using the
    /// default path-loss model.
    ///
    /// Proximity fingerprinting is the primary estimator; bounded trilateration
    /// is advisory and runs only when at least three placed anchors are present.
    #[must_use]
    pub fn estimate_position(&self, links: &[LinkObservation]) -> Scored<PositionEstimate> {
        self.estimate_position_with(links, &PathLossModel::default())
    }

    /// Estimate a position with an explicit path-loss model.
    #[must_use]
    pub fn estimate_position_with(
        &self,
        links: &[LinkObservation],
        model: &PathLossModel,
    ) -> Scored<PositionEstimate> {
        let valid: Vec<&LinkObservation> = links
            .iter()
            .filter(|l| self.node(l.tx).is_some() && self.node(l.rx).is_some())
            .collect();
        if valid.is_empty() {
            return Scored::unknown();
        }

        // Gather placed anchors: one sample per node, using its strongest
        // observed link as the coupling proxy.
        let mut anchor_rssi: HashMap<NodeId, f32> = HashMap::new();
        for l in &valid {
            for id in [l.tx, l.rx] {
                if self.node_position.get(&id).is_some() {
                    anchor_rssi
                        .entry(id)
                        .and_modify(|v| {
                            if l.rssi_dbm > *v {
                                *v = l.rssi_dbm;
                            }
                        })
                        .or_insert(l.rssi_dbm);
                }
            }
        }
        let mut samples: Vec<[f32; 3]> = Vec::with_capacity(anchor_rssi.len());
        for (id, rssi) in &anchor_rssi {
            if let Some(p) = self.node_position.get(id) {
                samples.push([p[0], p[1], *rssi]);
            }
        }

        let extent = self
            .model
            .zones
            .iter()
            .flat_map(|z| z.extent_home_m)
            .fold(0.0f32, f32::max)
            .max(DEFAULT_ZONE_EXTENT_M);

        if samples.len() >= 3 {
            if let Some(t) = trilaterate(&samples, model) {
                return self
                    .assemble_trilateration(t, extent, links);
            }
        }

        // Primary estimator only: zone-level, no coordinate.
        let estimate = PositionEstimate {
            method: PositionMethod::ProximityFingerprint,
            resolution: PositionResolution::Zone,
            zone: None,
            position_home_m: None,
            position_zone_m: None,
            error_ellipse_95_m: None,
            advisory: true,
            links_used: valid.iter().map(|l| (l.tx, l.rx)).collect(),
            confidence: Confidence::ZERO,
        };
        Scored::new(Some(estimate), Confidence::ZERO, false)
    }

    /// Fold a trilateration result into a [`PositionEstimate`], demoting to zone
    /// level when the 95 % ellipse exceeds the zone extent.
    fn assemble_trilateration(
        &self,
        t: crate::position::Trilateration,
        extent: f32,
        links: &[LinkObservation],
    ) -> Scored<PositionEstimate> {
        let major = t.error_ellipse_95.major_m;
        let within_zone = major <= extent;
        let ellipse_factor = (1.0 - (major / extent).clamp(0.0, 1.0)).max(0.0);
        let tri_conf = unit_confidence(
            (t.geometry_confidence.get() * ellipse_factor).min(TRILATERATION_CONFIDENCE_CAP),
        );
        let overall = if within_zone {
            tri_conf
        } else {
            crate::confidence::conjunction(Confidence::ZERO, tri_conf)
        };
        let position_zone_m = self.point_home_to_zone(t.position);
        let resolution = if within_zone {
            PositionResolution::SubZoneAdvisory
        } else {
            PositionResolution::Zone
        };
        let estimate = PositionEstimate {
            method: PositionMethod::TrilaterationSecondary,
            resolution,
            zone: None,
            position_home_m: Some(t.position),
            position_zone_m,
            error_ellipse_95_m: Some(ErrorEllipse {
                major_m: t.error_ellipse_95.major_m,
                minor_m: t.error_ellipse_95.minor_m,
                orientation_rad: t.error_ellipse_95.orientation_rad,
            }),
            advisory: true,
            links_used: links.iter().map(|l| (l.tx, l.rx)).collect(),
            confidence: overall,
        };
        Scored::new(Some(estimate), overall, false)
    }

    /// Find the zone containing a home-frame point, if any.
    fn point_home_to_zone(&self, p: [f32; 2]) -> Option<[f32; 2]> {
        self.model.zones.iter().find(|z| {
            (p[0] - z.origin_home_m[0]).abs() <= z.extent_home_m[0]
                && (p[1] - z.origin_home_m[1]).abs() <= z.extent_home_m[1]
        }).and_then(|z| {
            Some([p[0] - z.origin_home_m[0], p[1] - z.origin_home_m[1]])
        })
    }
}
