//! Home topology: the physical model from day one.
//!
//! **Home → Floors → Rooms → Nodes → Node-Pairs.** Stored as JSON in
//! `config/topology.json` and loaded by `aquila-geometry`. Nodes and rooms are
//! *configured*, not inferred. A zone is a room in v1 (`ZoneId == RoomId`).
//!
//! Coordinates are Cartesian, metres, right-handed, `x` east, `y` north, `z`
//! up. A floor carries `elevation_m`; a room carries `origin_home_m` and
//! `yaw_deg` mapping room-local to home frame; a node carries room-local
//! `pos_room_m`. Positions are `Option`: `None` means "not yet surveyed", and
//! geometry must degrade honestly rather than invent it.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::{invariant, require_finite_f32, require_non_empty, ValidationError};
use crate::ids::{FloorId, HomeId, MacAddr, NodeId, PairId, RoomId};
use crate::version::ContractVersion;

/// A calibrated dwelling: all its floors, rooms, nodes, node-pairs and its
/// global calibration. The unit of installation and of global calibration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Home {
    /// Identity of the home.
    pub id: HomeId,
    /// Human-readable name.
    pub name: String,
    /// IANA time zone, for example `"Australia/Sydney"`.
    pub time_zone: String,
    /// The contract version the topology was authored against.
    pub contract: ContractVersion,
}

impl Home {
    /// Validate identity and non-empty text fields.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.id.validate()?;
        require_non_empty("Home.name", &self.name)?;
        require_non_empty("Home.time_zone", &self.time_zone)
    }
}

/// A storey of a home, containing rooms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Floor {
    /// Identity of the floor.
    pub id: FloorId,
    /// The home this floor belongs to.
    pub home: HomeId,
    /// Human-readable name.
    pub name: String,
    /// The level: `-1` basement, `0` ground, `1`, `2`, and so on.
    pub level: i8,
    /// The height of this floor's plane in the home frame, in metres.
    pub elevation_m: f32,
}

impl Floor {
    /// Validate identity, ownership and finiteness.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.id.validate()?;
        self.home.validate()?;
        require_non_empty("Floor.name", &self.name)?;
        require_finite_f32("Floor.elevation_m", self.elevation_m)
    }
}

/// The kind of a room. `Unknown` is a first-class fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomKind {
    /// A living room.
    Living,
    /// A bedroom.
    Bedroom,
    /// A kitchen.
    Kitchen,
    /// A dining room.
    Dining,
    /// A bathroom.
    Bathroom,
    /// A hall or passage.
    Hall,
    /// A stairwell.
    Stairs,
    /// An office or study.
    Office,
    /// A utility room.
    Utility,
    /// An unrecognised or unset room kind.
    #[serde(other)]
    Unknown = 255,
}

/// A named physical partition of a floor.
///
/// In v1 a room is the spatial scope of a sensing result; a detection at room
/// scope is reported against a zone, and in v1 the zone is the room.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Room {
    /// Identity of the room.
    pub id: RoomId,
    /// The floor this room belongs to.
    pub floor: FloorId,
    /// Human-readable name.
    pub name: String,
    /// The kind of room.
    pub kind: RoomKind,
    /// The room-local origin in the home frame, in metres.
    pub origin_home_m: [f32; 3],
    /// The yaw of the room frame relative to the home frame, in degrees.
    pub yaw_deg: f32,
    /// The floor area in square metres, when surveyed.
    pub area_m2: Option<f32>,
}

impl Room {
    /// Validate identity, ownership and finiteness.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.id.validate()?;
        self.floor.validate()?;
        require_non_empty("Room.name", &self.name)?;
        for c in self.origin_home_m {
            require_finite_f32("Room.origin_home_m", c)?;
        }
        require_finite_f32("Room.yaw_deg", self.yaw_deg)?;
        if let Some(area) = self.area_m2 {
            require_finite_f32("Room.area_m2", area)?;
        }
        Ok(())
    }
}

/// The kind of node hardware. `Unknown` is a first-class fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    /// An ESP32-S3 node.
    Esp32S3,
    /// An ESP32-C6 node.
    Esp32C6,
    /// An unrecognised or unset device type.
    #[serde(other)]
    Unknown = 255,
}

/// A reference to a firmware build.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FirmwareRef {
    /// Human-readable firmware version.
    pub version: String,
    /// SHA-256 digest of the firmware image, or `None` when it is not known. An
    /// absent digest is `None`, never an all-zero hash that reads as real
    /// (`sister-issues-core.md` S14).
    pub sha256: Option<[u8; 32]>,
}

/// A node: one fixed ESP32 endpoint that streams raw CSI.
///
/// A dumb sensor: it performs no DSP, inference or fusion, and is assigned to
/// exactly one room.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// The dense identity of the node.
    pub id: NodeId,
    /// The room the node is assigned to.
    pub room: RoomId,
    /// Human-readable name.
    pub name: String,
    /// The factory MAC address.
    pub mac: MacAddr,
    /// The device hardware type.
    pub device: DeviceType,
    /// The firmware the node runs.
    pub firmware: FirmwareRef,
    /// Room-local position in metres; `None` until surveyed.
    pub pos_room_m: Option<[f32; 3]>,
    /// Unix wall time of installation, in milliseconds.
    pub installed_unix_ms: i64,
}

impl Node {
    /// Validate identity, ownership and finiteness.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.id.validate_provisioned()?;
        self.room.validate()?;
        require_non_empty("Node.name", &self.name)?;
        require_non_empty("Node.firmware.version", &self.firmware.version)?;
        if self.firmware.sha256 == Some([0u8; 32]) {
            return Err(ValidationError::Invariant {
                detail: "Node.firmware.sha256 must not be the all-zero hash".to_string(),
            });
        }
        if let Some(pos) = self.pos_room_m {
            for c in pos {
                require_finite_f32("Node.pos_room_m", c)?;
            }
        }
        Ok(())
    }
}

/// The scope of a node-pair, derived from the topology rather than chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairScope {
    /// Both nodes share a room.
    Room,
    /// Both nodes share a floor but not a room.
    SameFloor,
    /// The nodes are on different floors.
    CrossFloor,
}

/// An ordered node-pair: the unit of spatial sensing and the edge of the
/// sensing graph.
///
/// `tx` transmits; `rx` measures the channel from it. Direction is significant,
/// because CSI is directional. v1 stages pairs within-room first, then
/// within-floor, then cross-floor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodePair {
    /// Identity of the pair.
    pub id: PairId,
    /// The home this pair belongs to.
    pub home: HomeId,
    /// The transmitting node.
    pub tx: NodeId,
    /// The receiving node.
    pub rx: NodeId,
    /// The pair scope; derived, not free input.
    pub scope: PairScope,
}

impl NodePair {
    /// Validate identity and the `tx != rx` invariant.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.id.validate()?;
        self.home.validate()?;
        if self.tx == self.rx {
            return Err(invariant("NodePair.tx must differ from NodePair.rx"));
        }
        Ok(())
    }
}

/// The whole configured model. Referential integrity is an invariant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Topology {
    /// The home.
    pub home: Home,
    /// Its floors.
    pub floors: Vec<Floor>,
    /// Its rooms.
    pub rooms: Vec<Room>,
    /// Its nodes.
    pub nodes: Vec<Node>,
    /// Its ordered node-pairs.
    pub pairs: Vec<NodePair>,
}

impl Topology {
    /// Validate the whole model's referential integrity.
    ///
    /// Checks identifier uniqueness, ownership, node-pair direction and
    /// uniqueness, the `NodeId::UNPROVISIONED` ban, and that each stored
    /// `PairScope` agrees with the topology it is embedded in.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.home.validate()?;

        let mut floor_ids = HashSet::new();
        let mut levels = HashSet::new();
        for floor in &self.floors {
            floor.validate()?;
            if floor.home != self.home.id {
                return Err(invariant("Floor.home does not match Home.id"));
            }
            if !floor_ids.insert(floor.id.clone()) {
                return Err(invariant("duplicate Floor id"));
            }
            if !levels.insert(floor.level) {
                return Err(invariant("duplicate Floor.level within the home"));
            }
        }

        let mut room_ids = HashSet::new();
        for room in &self.rooms {
            room.validate()?;
            if !floor_ids.contains(&room.floor) {
                return Err(invariant("Room.floor does not exist"));
            }
            if !room_ids.insert(room.id.clone()) {
                return Err(invariant("duplicate Room id"));
            }
        }

        let mut node_ids = HashSet::new();
        for node in &self.nodes {
            node.validate()?;
            if !room_ids.contains(&node.room) {
                return Err(invariant("Node.room does not exist"));
            }
            if !node_ids.insert(node.id) {
                return Err(invariant("duplicate Node id"));
            }
        }

        let mut pair_ids = HashSet::new();
        let mut ordered_pairs = HashSet::new();
        for pair in &self.pairs {
            pair.validate()?;
            if pair.home != self.home.id {
                return Err(invariant("NodePair.home does not match Home.id"));
            }
            if !node_ids.contains(&pair.tx) || !node_ids.contains(&pair.rx) {
                return Err(invariant("NodePair endpoint does not exist"));
            }
            if !pair_ids.insert(pair.id.clone()) {
                return Err(invariant("duplicate NodePair id"));
            }
            if !ordered_pairs.insert((pair.tx, pair.rx)) {
                return Err(invariant("duplicate ordered NodePair (tx, rx)"));
            }
            let derived = self.derive_scope(pair.tx, pair.rx);
            if pair.scope != derived {
                return Err(invariant("NodePair.scope disagrees with the topology"));
            }
        }

        Ok(())
    }

    /// Derive the scope of an ordered node-pair from the topology.
    ///
    /// Returns [`PairScope::Room`] when both nodes share a room,
    /// [`PairScope::SameFloor`] when they share a floor but not a room, and
    /// [`PairScope::CrossFloor`] otherwise. Returns `CrossFloor` when either
    /// endpoint is unknown, which validation rejects separately.
    #[must_use]
    pub fn derive_scope(&self, tx: NodeId, rx: NodeId) -> PairScope {
        let room_of = |node: NodeId| self.nodes.iter().find(|n| n.id == node).map(|n| &n.room);
        let floor_of = |room: &RoomId| self.rooms.iter().find(|r| &r.id == room).map(|r| &r.floor);
        match (room_of(tx), room_of(rx)) {
            (Some(tx_room), Some(rx_room)) if tx_room == rx_room => PairScope::Room,
            (Some(tx_room), Some(rx_room)) => match (floor_of(tx_room), floor_of(rx_room)) {
                (Some(tx_floor), Some(rx_floor)) if tx_floor == rx_floor => PairScope::SameFloor,
                _ => PairScope::CrossFloor,
            },
            _ => PairScope::CrossFloor,
        }
    }
}
