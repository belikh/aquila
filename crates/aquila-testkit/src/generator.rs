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
    CsiEnvelope, DeviceType, FirmwareRef, GroundTruthRecord, Home, MacAddr, Node, Pose,
    Slug, Topology, Zone, ZoneId, CONTRACT_VERSION,
};

/// A zone plan for hardware bring-up tests: defines a zone and the zones it contains.
#[derive(Debug, Clone, PartialEq)]
pub struct ZonePlan {
    pub id: Slug,
    pub name: String,
    pub zones: Vec<ZoneId>,
}
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
    let firmware = FirmwareRef {
        version: "tracer".to_string(),
        sha256: None,
    };

    let mut zones: Vec<(ZoneId, [f32; 3])> = Vec::new();
    for node in &script.nodes {
        if !zones.iter().any(|(z, _)| z == &node.zone) {
            zones.push((node.zone.clone(), [node.pos_m[0] as f32, node.pos_m[1] as f32, node.pos_m[2] as f32]));
        }
    }

    Topology {
        home: Home {
            id: home.clone(),
            name: script.name.clone(),
            time_zone: "Australia/Sydney".to_string(),
            contract: CONTRACT_VERSION,
        },
        zones: zones
            .iter()
            .enumerate()
            .map(|(i, (zone, origin))| Zone {
                id: zone.clone(),
                origin_home_m: [
                    origin[0] as f32,
                    origin[1] as f32,
                    origin[2] as f32,
                ],
                extent_home_m: [1.0, 1.0, 1.0],
                name: zone.as_str().to_string(),
            })
            .collect(),
        nodes: script
            .nodes
            .iter()
            .map(|spec| Node {
                id: spec.id,
                zone: spec.zone.clone(),
                name: format!("node-{}", spec.id.0),
                mac: MacAddr([0, 0, 0, 0, 0, spec.id.0 as u8]),
                device: DeviceType::Esp32S3,
                firmware: firmware.clone(),
                pos_home_m: [
                    spec.pos_m[0] as f32,
                    spec.pos_m[1] as f32,
                    spec.pos_m[2] as f32,
                ],
                installed_unix_ms: 0,
            })
            .collect(),
        pairs: vec![],
    }
}

pub fn topology_for_floors(script: &SceneScript, plans: &[ZonePlan]) -> Topology {
    let home = script.home.clone();
    let firmware = FirmwareRef {
        version: "tracer".to_string(),
        sha256: None,
    };

    let mut zones: Vec<(ZoneId, [f32; 3])> = Vec::new();
    for plan in plans {
        for zone_id in &plan.zones {
            if !zones.iter().any(|(z, _)| z == zone_id) {
                if let Some(node) = script.nodes.iter().find(|n| n.zone == *zone_id) {
                    zones.push((zone_id.clone(), [node.pos_m[0] as f32, node.pos_m[1] as f32, node.pos_m[2] as f32]));
                } else {
                    zones.push((zone_id.clone(), [0.0, 0.0, 2.4]));
                }
            }
        }
    }

    Topology {
        home: Home {
            id: home.clone(),
            name: script.name.clone(),
            time_zone: "Australia/Sydney".to_string(),
            contract: CONTRACT_VERSION,
        },
        zones: zones
            .iter()
            .enumerate()
            .map(|(i, (zone, origin))| Zone {
                id: zone.clone(),
                extent_home_m: [1.0, 1.0, 1.0],
                origin_home_m: [
                    origin[0] as f32,
                    origin[1] as f32,
                    origin[2] as f32,
                ],
                name: zone.as_str().to_string(),
            })
            .collect(),
        nodes: script
            .nodes
            .iter()
            .map(|spec| Node {
                id: spec.id,
                zone: spec.zone.clone(),
                name: format!("node-{}", spec.id.0),
                mac: MacAddr([0, 0, 0, 0, 0, spec.id.0 as u8]),
                device: DeviceType::Esp32S3,
                firmware: firmware.clone(),
                pos_home_m: [
                    spec.pos_m[0] as f32,
                    spec.pos_m[1] as f32,
                    spec.pos_m[2] as f32,
                ],
                installed_unix_ms: 0,
            })
            .collect(),
        pairs: vec![],
    }
}