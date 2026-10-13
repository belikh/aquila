//! The indexed home topology: the deep module every geometry query goes
//! through.
//!
//! [`Topology`] wraps the contract `aquila_types::Topology`
//! (`data-contracts.md` §3) with precomputed lookup indices, validates the
//! model once at [`Topology::load`], and answers the register's queries:
//! attribution ([`Topology::room_of`], [`Topology::zone_of`],
//! [`Topology::floor_of`], [`Topology::position`]), link scope
//! ([`Topology::scope_of`], [`Topology::pairs`]) and advisory position
//! ([`Topology::estimate_position`]).
//!
//! It performs no I/O: JSON decoding stays with the caller, so geometry remains
//! pure computation (`geometry-calibration.md` §6).

use std::cmp::Ordering;
use std::collections::HashMap;

use aquila_types::{
    Confidence, Floor, FloorId, Home, Node, NodeId, NodePair, PairScope, Room, RoomId, ZoneId,
};

use crate::confidence::{independent, unit_confidence, Scored};
use crate::error::GeometryError;
use crate::frame::{Frame, Transform2};
use crate::position::{
    link_confidence, trilaterate, ErrorEllipse, PathLossModel, PositionEstimate, PositionMethod,
    PositionResolution, AMBIGUITY_MARGIN, DEFAULT_ROOM_EXTENT_M, TRILATERATION_CONFIDENCE_CAP,
};
use crate::proximity::{relation_for_scope, LinkObservation, ProximityRelation};

/// An indexed, validated home topology.
///
/// Built from the contract model by [`Topology::load`]. Queries are total over
/// nodes that belong to the topology and honest (`Scored::unknown`) for
/// anything else.
#[derive(Debug, Clone)]
pub struct Topology {
    model: aquila_types::Topology,
    room_index: HashMap<RoomId, usize>,
    floor_index: HashMap<FloorId, usize>,
    node_index: HashMap<NodeId, usize>,
    node_room: HashMap<NodeId, RoomId>,
    room_floor: HashMap<RoomId, FloorId>,
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

        let mut room_index = HashMap::with_capacity(model.rooms.len());
        for (i, room) in model.rooms.iter().enumerate() {
            room_index.insert(room.id.clone(), i);
        }
        let mut floor_index = HashMap::with_capacity(model.floors.len());
        for (i, floor) in model.floors.iter().enumerate() {
            floor_index.insert(floor.id.clone(), i);
        }
        let mut node_index = HashMap::with_capacity(model.nodes.len());
        let mut node_room = HashMap::with_capacity(model.nodes.len());
        for (i, node) in model.nodes.iter().enumerate() {
            node_index.insert(node.id, i);
            node_room.insert(node.id, node.room.clone());
        }
        let mut room_floor = HashMap::with_capacity(model.rooms.len());
        for room in &model.rooms {
            room_floor.insert(room.id.clone(), room.floor.clone());
        }

        Ok(Self {
            model,
            room_index,
            floor_index,
            node_index,
            node_room,
            room_floor,
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

    /// Look up a room.
    #[must_use]
    pub fn room(&self, room: &RoomId) -> Option<&Room> {
        self.room_index.get(room).map(|&i| &self.model.rooms[i])
    }

    /// Look up a floor.
    #[must_use]
    pub fn floor(&self, floor: &FloorId) -> Option<&Floor> {
        self.floor_index.get(floor).map(|&i| &self.model.floors[i])
    }

    /// The ordered node-pairs of the sensing graph, as authored.
    #[must_use]
    pub fn pairs(&self) -> &[NodePair] {
        &self.model.pairs
    }

    /// Attribute a node to its room.
    ///
    /// Total for nodes that belong to the topology: a configured mount is
    /// certain, so a known node yields [`Confidence::ONE`]. An unknown node
    /// yields [`Scored::unknown`].
    #[must_use]
    pub fn room_of(&self, node: NodeId) -> Scored<&RoomId> {
        match self.node_room.get(&node) {
            Some(room) => Scored::certain(room),
            None => Scored::unknown(),
        }
    }

    /// Attribute a node to its zone, best-effort.
    ///
    /// In v1 a zone *is* a room (`ZoneId == RoomId`), so a known node yields
    /// `Some(room)`. The `Option` expresses the future case where no zone
    /// separates cleanly (`geometry-calibration.md` §4).
    #[must_use]
    pub fn zone_of(&self, node: NodeId) -> Scored<Option<&ZoneId>> {
        match self.node_room.get(&node) {
            Some(room) => Scored::certain(Some(room)),
            None => Scored::unknown(),
        }
    }

    /// Attribute a node to its floor, via its room.
    #[must_use]
    pub fn floor_of(&self, node: NodeId) -> Scored<&FloorId> {
        match self
            .node_room
            .get(&node)
            .and_then(|room| self.room_floor.get(room))
        {
            Some(floor) => Scored::certain(floor),
            None => Scored::unknown(),
        }
    }

    /// The surveyed room-local position of a node.
    ///
    /// Known and placed yields [`Confidence::ONE`]; an unknown node or an
    /// unsurveyed position yields [`Scored::unknown`]. The value is the stored
    /// survey, not an inference; its physical accuracy is not modelled here.
    #[must_use]
    pub fn position(&self, node: NodeId) -> Scored<[f32; 3]> {
        match self.node(node).and_then(|n| n.pos_room_m) {
            Some(p) => Scored::certain(p),
            None => Scored::unknown(),
        }
    }

    /// Derive the scope of an unordered node pair from the topology.
    ///
    /// Returns [`PairScope::Room`] when both nodes share a room,
    /// [`PairScope::SameFloor`] when they share a floor but not a room, and
    /// [`PairScope::CrossFloor`] otherwise. `None` when either endpoint is not
    /// in the topology. Scope is **symmetric**: `scope_of(a, b) == scope_of(b, a)`.
    #[must_use]
    pub fn scope_of(&self, tx: NodeId, rx: NodeId) -> Option<PairScope> {
        let tx_room = self.node_room.get(&tx)?;
        let rx_room = self.node_room.get(&rx)?;
        Some(if tx_room == rx_room {
            PairScope::Room
        } else if self.room_floor.get(tx_room) == self.room_floor.get(rx_room) {
            PairScope::SameFloor
        } else {
            PairScope::CrossFloor
        })
    }

    /// The discrete proximity relation of a link, from its topology scope; the
    /// v1 primary-estimator prior. `None` when either endpoint is unknown.
    #[must_use]
    pub fn proximity_relation(&self, tx: NodeId, rx: NodeId) -> Option<ProximityRelation> {
        self.scope_of(tx, rx).map(relation_for_scope)
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

    /// The transform placing a room frame in the home frame, when the room
    /// exists.
    #[must_use]
    pub fn room_frame(&self, room: &RoomId) -> Option<Transform2> {
        self.room(room).map(Transform2::from_room)
    }

    /// The transform placing a floor frame in the home frame, when the floor
    /// exists.
    ///
    /// A floor shares the home `x`/`y` axes and differs only by elevation,
    /// which lives in `z` and is therefore not part of this planar transform.
    #[must_use]
    pub fn floor_frame(&self, floor: &FloorId) -> Option<Transform2> {
        self.floor(floor).map(|_| Transform2::identity())
    }

    /// The transform placing a node mount frame in the home frame, when the
    /// node and its room exist and its position is surveyed.
    ///
    /// An unsurveyed or unknown node yields [`Scored::unknown`] — never a
    /// guessed transform (`geometry-calibration.md` §2).
    #[must_use]
    pub fn node_frame(&self, node: NodeId) -> Scored<Transform2> {
        let Some(n) = self.node(node) else {
            return Scored::unknown();
        };
        let Some(room) = self.room(&n.room) else {
            return Scored::unknown();
        };
        let Some(p) = n.pos_room_m else {
            return Scored::unknown();
        };
        let room_frame = Transform2::from_room(room);
        let node_in_room = Transform2::new([p[0], p[1]], 0.0, room_frame.confidence);
        Scored::certain(room_frame.compose(&node_in_room))
    }

    /// Resolve a named frame to its transform in the home frame.
    #[must_use]
    pub fn frame_transform(&self, frame: &Frame) -> Scored<Transform2> {
        match frame {
            Frame::World => Scored::certain(Transform2::identity()),
            Frame::Floor(floor) => match self.floor_frame(floor) {
                Some(t) => Scored::certain(t),
                None => Scored::unknown(),
            },
            Frame::Room(room) => match self.room_frame(room) {
                Some(t) => Scored::certain(t),
                None => Scored::unknown(),
            },
            Frame::Node(node) => self.node_frame(*node),
        }
    }

    /// Map a room-local planar point into the home frame.
    #[must_use]
    pub fn point_room_to_home(&self, room: &RoomId, p: [f32; 2]) -> Option<[f32; 2]> {
        self.room_frame(room).map(|f| f.apply(p))
    }

    /// Map a home-frame planar point into a room's local frame.
    #[must_use]
    pub fn point_home_to_room(&self, room: &RoomId, p: [f32; 2]) -> Option<[f32; 2]> {
        self.room_frame(room).map(|f| f.apply_inverse(p))
    }

    /// A node's surveyed position in the home frame, when known.
    #[must_use]
    pub fn node_home_position(&self, node: NodeId) -> Option<[f32; 3]> {
        let n = self.node(node)?;
        let p = n.pos_room_m?;
        let room = self.room(&n.room)?;
        let frame = Transform2::from_room(room);
        let planar = frame.apply([p[0], p[1]]);
        Some([planar[0], planar[1], room.origin_home_m[2] + p[2]])
    }

    /// The planar part of a node's surveyed home-frame position.
    #[must_use]
    pub fn node_home_xy(&self, node: NodeId) -> Option<[f32; 2]> {
        self.node_home_position(node).map(|p| [p[0], p[1]])
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

        let (room, room_conf, ambiguous) = self.attribute_room(&valid);
        let links_used: Vec<(NodeId, NodeId)> = valid.iter().map(|l| (l.tx, l.rx)).collect();

        // Gather placed anchors: one sample per node, using its strongest
        // observed link as the coupling proxy.
        let mut anchor_rssi: HashMap<NodeId, f32> = HashMap::new();
        for l in &valid {
            for id in [l.tx, l.rx] {
                if self.node_home_xy(id).is_some() {
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
            if let Some(p) = self.node_home_xy(*id) {
                samples.push([p[0], p[1], *rssi]);
            }
        }

        let extent = room
            .as_ref()
            .and_then(|r| self.room(r))
            .and_then(|r| r.area_m2)
            .map(|a| a.max(0.0).sqrt())
            .filter(|e| e.is_finite() && *e > 0.0)
            .unwrap_or(DEFAULT_ROOM_EXTENT_M);

        if samples.len() >= 3 {
            if let Some(t) = trilaterate(&samples, model) {
                return self
                    .assemble_trilateration(t, room, room_conf, ambiguous, links_used, extent);
            }
        }

        // Primary estimator only: room-level, no coordinate.
        let estimate = PositionEstimate {
            method: PositionMethod::ProximityFingerprint,
            resolution: PositionResolution::Room,
            room: room.clone(),
            position_home_m: None,
            position_room_m: None,
            error_ellipse_95_m: None,
            advisory: true,
            links_used,
            confidence: room_conf,
        };
        Scored::new(Some(estimate), room_conf, ambiguous)
    }

    /// Fold a trilateration result into a [`PositionEstimate`], demoting to room
    /// level when the 95 % ellipse exceeds the room extent.
    fn assemble_trilateration(
        &self,
        t: crate::position::Trilateration,
        room: Option<RoomId>,
        room_conf: Confidence,
        ambiguous: bool,
        links_used: Vec<(NodeId, NodeId)>,
        extent: f32,
    ) -> Scored<PositionEstimate> {
        let major = t.error_ellipse_95.major_m;
        let within_room = major <= extent;
        let ellipse_factor = (1.0 - (major / extent).clamp(0.0, 1.0)).max(0.0);
        let tri_conf = unit_confidence(
            (t.geometry_confidence.get() * ellipse_factor).min(TRILATERATION_CONFIDENCE_CAP),
        );
        let overall = if within_room {
            independent(room_conf, tri_conf)
        } else {
            crate::confidence::conjunction(room_conf, tri_conf)
        };
        let position_room_m = room
            .as_ref()
            .and_then(|r| self.point_home_to_room(r, t.position));
        let resolution = if within_room {
            PositionResolution::SubRoomAdvisory
        } else {
            PositionResolution::Room
        };
        let estimate = PositionEstimate {
            method: PositionMethod::TrilaterationSecondary,
            resolution,
            room,
            position_home_m: Some(t.position),
            position_room_m,
            error_ellipse_95_m: Some(ErrorEllipse {
                major_m: t.error_ellipse_95.major_m,
                minor_m: t.error_ellipse_95.minor_m,
                orientation_rad: t.error_ellipse_95.orientation_rad,
            }),
            advisory: true,
            links_used,
            confidence: overall,
        };
        Scored::new(Some(estimate), overall, ambiguous)
    }

    /// Feature-level proximity aggregation: each link votes for the rooms of its
    /// endpoints with a per-link weight, votes combine by a noisy-OR, and the
    /// top two within [`AMBIGUITY_MARGIN`] are reported as ambiguous
    /// (`geometry-calibration.md` §4).
    fn attribute_room(&self, links: &[&LinkObservation]) -> (Option<RoomId>, Confidence, bool) {
        let mut votes: HashMap<RoomId, f32> = HashMap::new();
        for l in links {
            let weight = link_confidence(l.rssi_dbm).get();
            let mut seen: Vec<&RoomId> = Vec::new();
            for id in [l.tx, l.rx] {
                if let Some(room) = self.node_room.get(&id) {
                    if !seen.contains(&room) {
                        seen.push(room);
                    }
                }
            }
            for room in seen {
                let entry = votes.entry(room.clone()).or_insert(0.0);
                *entry = 1.0 - (1.0 - *entry) * (1.0 - weight);
            }
        }
        if votes.is_empty() {
            return (None, Confidence::ZERO, false);
        }
        let mut ranked: Vec<(RoomId, f32)> = votes.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        let (top_room, top_score) = ranked[0].clone();
        let ambiguous = ranked
            .get(1)
            .is_some_and(|second| (top_score - second.1).abs() < AMBIGUITY_MARGIN);
        (Some(top_room), unit_confidence(top_score), ambiguous)
    }
}
