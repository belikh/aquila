//! Home topology: the physical model from day one.
//!
//! **Home → Zones → Nodes → NodePairs.** Stored as JSON in
//! `config/topology.json` and loaded by `aquila-geometry`. Zones are virtual
//! detection regions in 3D home space and are the only spatial concept; there
//! are no rooms and no floors. Nodes are dumb sensors at home-frame positions.
//! Zones and nodes are *configured*, not inferred.
//!
//! Coordinates are Cartesian, metres, right-handed, `x` east, `y` north, `z`
//! up. A zone carries `origin_home_m` and `extent_home_m`; a node carries
//! `pos_home_m`. Geometry must degrade honestly rather than invent positions.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::{invariant, require_finite_f32, require_non_empty, ValidationError};
use crate::ids::{HomeId, MacAddr, NodeId, PairId, ZoneId};
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

/// A virtual detection region in 3D home space. A zone is any region the
/// operator draws — for example "bed", "shower", "toilet", "lounge", or
/// "whole home". Zones may overlap; a person is attributed to a zone by the
/// fused pipeline from all node-pairs, not by which room a node is in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    /// Identity of the zone.
    pub id: ZoneId,
    /// Human-readable name, for example `"bed"` or `"shower"`.
    pub name: String,
    /// The zone's origin in the home frame, in metres.
    pub origin_home_m: [f32; 3],
    /// The zone's half-extents in x/y/z, in metres. A point is inside the zone
    /// when `|p - origin| <= extent` component-wise.
    pub extent_home_m: [f32; 3],
}

impl Zone {
    /// Validate identity and finiteness.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.id.validate()?;
        require_non_empty("Zone.name", &self.name)?;
        for c in self.origin_home_m {
            require_finite_f32("Zone.origin_home_m", c)?;
        }
        for c in self.extent_home_m {
            require_finite_f32("Zone.extent_home_m", c)?;
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
/// A dumb sensor: it performs no DSP, inference or fusion. Nodes are not "in"
/// a zone; they contribute CSI to zone detection collaboratively via
/// node-pairs. The node's position is its antenna mount point in home frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// The dense identity of the node.
    pub id: NodeId,
    /// Human-readable name.
    pub name: String,
    /// The factory MAC address.
    pub mac: MacAddr,
    /// The device hardware type.
    pub device: DeviceType,
    /// The firmware the node runs.
    pub firmware: FirmwareRef,
    /// The node's antenna mount point in home frame, in metres.
    pub pos_home_m: [f32; 3],
    /// Unix wall time of installation, in milliseconds.
    pub installed_unix_ms: i64,
}

impl Node {
    /// Validate identity and finiteness.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.id.validate_provisioned()?;
        require_non_empty("Node.name", &self.name)?;
        require_non_empty("Node.firmware.version", &self.firmware.version)?;
        if self.firmware.sha256 == Some([0u8; 32]) {
            return Err(ValidationError::Invariant {
                detail: "Node.firmware.sha256 must not be the all-zero hash".to_string(),
            });
        }
        for c in self.pos_home_m {
            require_finite_f32("Node.pos_home_m", c)?;
        }
        Ok(())
    }
}


/// An ordered node-pair: the unit of spatial sensing and the edge of the
/// sensing graph.
///
/// `tx` transmits; `rx` measures the channel from it. Direction is significant,
/// because CSI is directional. A pair is any two nodes whose CSI is jointly
/// processed; scope (near/far, same zone / different zone) is inferred at
/// runtime from positions, not authored as topology.
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
///
/// **Home → Zones → Nodes → NodePairs.** There are no floors and no rooms;
/// zones are the only spatial concept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Topology {
    /// The home.
    pub home: Home,
    /// Its zones: virtual detection regions in 3D home space. Zones are the
    /// unit of sensing output.
    pub zones: Vec<Zone>,
    /// Its nodes: dumb sensors at home-frame positions.
    pub nodes: Vec<Node>,
    /// Its ordered node-pairs.
    pub pairs: Vec<NodePair>,
}

impl Topology {
    /// Validate the whole model's referential integrity.
    ///
    /// Checks identifier uniqueness, ownership, node-pair direction and
    /// uniqueness, and the `NodeId::UNPROVISIONED` ban.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.home.validate()?;

        let mut zone_ids = HashSet::new();
        for zone in &self.zones {
            zone.validate()?;
            if !zone_ids.insert(zone.id.clone()) {
                return Err(invariant("duplicate Zone id"));
            }
        }

        let mut node_ids = HashSet::new();
        for node in &self.nodes {
            node.validate()?;
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
        }

        Ok(())
    }
}
