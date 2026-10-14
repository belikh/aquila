//! The deterministic CSI generator.
//!
//! A thin, in-tree wrapper over `aquila-simulator`: given a scene script and a
//! [`SimConfig`], it produces reproducible CSI frames, a reproducible `.aqs`
//! session and the synthesised ground truth. It is the "drive a generator" half
//! of the testkit contract (`testing.md` §3); the replay half loads the session
//! through the real wire codec.

use aquila_simulator::{synthesise_session, SceneScript, SimConfig, SimError, SimulatorSource};
use aquila_sources::{CsiSource, FrameOrEnd, SourceError};
use aquila_types::{
    CsiEnvelope, DeviceType, FirmwareRef, Floor, GroundTruthRecord, Home, MacAddr, Node, Pose,
    Room, RoomId, RoomKind, Slug, Topology, CONTRACT_VERSION,
};

/// A seeded, reproducible CSI synthesiser for one scene.
#[derive(Debug, Clone)]
pub struct DeterministicCsiGenerator {
    script: SceneScript,
    config: SimConfig,
}

impl DeterministicCsiGenerator {
    /// Construct a generator, validating the scene eagerly.
    ///
    /// # Errors
    ///
    /// Returns [`SimError`] when the scene fails validation.
    pub fn new(script: SceneScript, config: SimConfig) -> Result<Self, SimError> {
        // Construct and drop a source once so validation happens here, not on
        // first use.
        let _ = SimulatorSource::new(script.clone(), config.clone())?;
        Ok(Self { script, config })
    }

    /// The scene being generated.
    #[must_use]
    pub const fn script(&self) -> &SceneScript {
        &self.script
    }

    /// The generation configuration.
    #[must_use]
    pub const fn config(&self) -> &SimConfig {
        &self.config
    }

    /// Drain the generator into its decoded envelopes.
    ///
    /// # Errors
    ///
    /// Returns a typed [`SourceError`] if the simulator cannot be built.
    pub fn frames(&self) -> Result<Vec<CsiEnvelope>, SourceError> {
        let mut source = SimulatorSource::new(self.script.clone(), self.config.clone())
            .map_err(|error| SourceError::Other(error.to_string()))?;
        let mut frames = Vec::new();
        while let FrameOrEnd::Frame(envelope) = source.recv()? {
            frames.push(envelope);
        }
        Ok(frames)
    }

    /// Synthesise a complete `.aqs` session.
    ///
    /// # Errors
    ///
    /// Returns [`SimError`] if the scene is invalid or the session cannot be
    /// written.
    pub fn session_bytes(&self) -> Result<Vec<u8>, SimError> {
        synthesise_session(&self.script, &self.config)
    }

    /// The synthesised ground-truth stream (research-only; never a runtime
    /// input).
    #[must_use]
    pub fn ground_truth(&self) -> Vec<(u64, GroundTruthRecord)> {
        SimulatorSource::new(self.script.clone(), self.config.clone())
            .map(|source| source.ground_truth())
            .unwrap_or_default()
    }

    /// The synthesised coarse-pose ground-truth stream (research-only; never a
    /// runtime input).
    #[must_use]
    pub fn pose_ground_truth(&self) -> Vec<Pose> {
        SimulatorSource::new(self.script.clone(), self.config.clone())
            .map(|source| source.pose_ground_truth())
            .unwrap_or_default()
    }
}

/// Build a minimal contract topology for a scene: one floor, one room per
/// distinct node room, and one node per scene node.
///
/// The scene carries only what the simulator needs (node id and room), so this
/// fills the remaining contract fields with deterministic placeholders. It keeps
/// a tracer test free of hand-written topology boilerplate.
#[must_use]
pub fn topology_for(script: &SceneScript) -> Topology {
    let home = script.home.clone();
    let floor: Slug = Slug("ground".to_string());
    let mut rooms: Vec<RoomId> = Vec::new();
    for node in &script.nodes {
        if !rooms.contains(&node.room) {
            rooms.push(node.room.clone());
        }
    }

    let firmware = FirmwareRef {
        version: "tracer".to_string(),
        sha256: None,
    };

    Topology {
        home: Home {
            id: home.clone(),
            name: script.name.clone(),
            time_zone: "Australia/Sydney".to_string(),
            contract: CONTRACT_VERSION,
        },
        floors: vec![Floor {
            id: floor.clone(),
            home: home.clone(),
            name: "Ground".to_string(),
            level: 0,
            elevation_m: 0.0,
        }],
        rooms: rooms
            .iter()
            .map(|room| Room {
                id: room.clone(),
                floor: floor.clone(),
                name: room.as_str().to_string(),
                kind: RoomKind::Unknown,
                origin_home_m: [0.0, 0.0, 0.0],
                yaw_deg: 0.0,
                area_m2: None,
            })
            .collect(),
        nodes: script
            .nodes
            .iter()
            .map(|spec| Node {
                id: spec.id,
                room: spec.room.clone(),
                name: format!("node-{}", spec.id.0),
                mac: MacAddr([0, 0, 0, 0, 0, spec.id.0 as u8]),
                device: DeviceType::Esp32S3,
                firmware: firmware.clone(),
                pos_room_m: Some([
                    spec.pos_m[0] as f32,
                    spec.pos_m[1] as f32,
                    spec.pos_m[2] as f32,
                ]),
                installed_unix_ms: 0,
            })
            .collect(),
        pairs: vec![],
    }
}

/// A floor plan for [`topology_for_floors`]: one floor and the rooms it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct FloorPlan {
    /// The floor identity.
    pub id: Slug,
    /// Human-readable floor name.
    pub name: String,
    /// The level (`0` ground, `1`, `2`, and so on).
    pub level: i8,
    /// The floor plane's elevation in the home frame, in metres.
    pub elevation_m: f32,
    /// The rooms on this floor, in canonical order.
    pub rooms: Vec<RoomId>,
}

/// Build a contract topology with an explicit multi-floor hierarchy
/// (`roadmap.md`, Slice 6).
///
/// Every room named in `floors` becomes a `Room` on its floor, and the scene's
/// nodes attach to their declared rooms. A scene room not named in any plan is
/// placed on the first floor, deterministically, so the helper never fails or
/// panics; a caller that wants a strict mapping should cover every scene room.
/// With no plans, a single `ground` floor is synthesised, matching
/// [`topology_for`].
#[must_use]
pub fn topology_for_floors(script: &SceneScript, floors: &[FloorPlan]) -> Topology {
    let home = script.home.clone();
    let firmware = FirmwareRef {
        version: "tracer".to_string(),
        sha256: None,
    };

    let mut floor_records: Vec<Floor> = floors
        .iter()
        .map(|plan| Floor {
            id: plan.id.clone(),
            home: home.clone(),
            name: plan.name.clone(),
            level: plan.level,
            elevation_m: plan.elevation_m,
        })
        .collect();
    if floor_records.is_empty() {
        floor_records.push(Floor {
            id: Slug("ground".to_string()),
            home: home.clone(),
            name: "Ground".to_string(),
            level: 0,
            elevation_m: 0.0,
        });
    }
    let first_floor = floor_records[0].id.clone();

    // Rooms from the plans, first-seen order, de-duplicated.
    let mut rooms: Vec<Room> = Vec::new();
    let mut seen: Vec<RoomId> = Vec::new();
    for plan in floors {
        for room in &plan.rooms {
            if seen.contains(room) {
                continue;
            }
            seen.push(room.clone());
            rooms.push(Room {
                id: room.clone(),
                floor: plan.id.clone(),
                name: room.as_str().to_string(),
                kind: RoomKind::Unknown,
                origin_home_m: [0.0, 0.0, 0.0],
                yaw_deg: 0.0,
                area_m2: None,
            });
        }
    }
    // Any scene room not covered by a plan is placed on the first floor.
    for node in &script.nodes {
        if !seen.contains(&node.room) {
            seen.push(node.room.clone());
            rooms.push(Room {
                id: node.room.clone(),
                floor: first_floor.clone(),
                name: node.room.as_str().to_string(),
                kind: RoomKind::Unknown,
                origin_home_m: [0.0, 0.0, 0.0],
                yaw_deg: 0.0,
                area_m2: None,
            });
        }
    }

    Topology {
        home: Home {
            id: home,
            name: script.name.clone(),
            time_zone: "Australia/Sydney".to_string(),
            contract: CONTRACT_VERSION,
        },
        floors: floor_records,
        rooms,
        nodes: script
            .nodes
            .iter()
            .map(|spec| Node {
                id: spec.id,
                room: spec.room.clone(),
                name: format!("node-{}", spec.id.0),
                mac: MacAddr([0, 0, 0, 0, 0, spec.id.0 as u8]),
                device: DeviceType::Esp32S3,
                firmware: firmware.clone(),
                pos_room_m: Some([
                    spec.pos_m[0] as f32,
                    spec.pos_m[1] as f32,
                    spec.pos_m[2] as f32,
                ]),
                installed_unix_ms: 0,
            })
            .collect(),
        pairs: vec![],
    }
}
