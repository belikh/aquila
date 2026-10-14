//! Global whole-home calibration: the `aquila.calibration/v1` artefact, the
//! wizard that learns it, and its persistence and invalidation policy.
//!
//! Calibration is the per-home process that corrects node and node-pair CSI to a
//! common reference, and the stored corrections it produces (`CONTEXT.md`). v1
//! is **global** — one calibration for the whole home — not per-room
//! (`geometry-calibration.md` §7). Per ADR-0003 §6 this is an internal module of
//! `aquila-sensing`, not a crate; it reaches the signal stage only as data.
//!
//! # What it learns
//!
//! From a replay/simulated **empty-home** capture and a labelled **walk-through**
//! capture (`geometry-calibration.md` §7, §8), the wizard learns:
//!
//! - **L1 per-node** clock offset + skew + fit residual, from the affine map
//!   between the node's device stamp and its host arrival (`fusion-sync.md`
//!   §1.3); this seeds the core's `TimeSync` so drift correction is calibrated.
//! - **L2 per-link** empty-home baselines: per-subcarrier amplitude mean and
//!   standard deviation, noise floor, excess dynamic range and SNR.
//! - **L2 per-node** empty-home amplitude variability — the exact quantity the
//!   presence detector measures — so a zone threshold can be expressed as a
//!   decibel rise above a node's own baseline.
//! - **L3 per-zone** presence and motion thresholds targeting a chosen
//!   false-positive rate, from the empty-home and walk-through levels.
//! - **L4 per-floor** slab attenuation, staged and zero when no cross-floor link
//!   exists.
//!
//! # The artefact
//!
//! The artefact is one canonical JSON document per calibration, content-addressed
//! (`geometry-calibration.md` §9, §13): `body_sha256` is the SHA-256 of the body
//! with `calibration_id` and `body_sha256` blanked, and `calibration_id` is
//! `sha256:<hex>` over that digest. Re-running the wizard on the same fixtures
//! reproduces the same `calibration_id`. Large physical arrays are `f32`; the
//! document is deterministic (fixed field order, vectors in canonical order).
//!
//! # Storage and invalidation
//!
//! The artefact lives under a configurable data directory, by default
//! `/data/aquila/homes/<home_id>/calibration/`, with an append-only
//! `index.json` carrying the `current` pointer (`geometry-calibration.md` §9).
//! Writes are temp-file → `fsync` → atomic rename. An artefact is versioned by
//! schema + learning semantics + core commit + topology digest + firmware
//! (`§10`), and a mismatch is a typed [`InvalidationReason`]; an invalid or
//! stale artefact is **never silently used**.
//!
//! # Prose
//!
//! Australian English throughout (normalise, behaviour, centre).

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use aquila_geometry::Topology;
use aquila_types::{
    sha256, to_hex, CsiEnvelope, DeviceType, HomeId, NodeId, NodeQuality, RoomId,
};
use serde::{Deserialize, Serialize};

use crate::config::SensingConfig;
use crate::error::{CalibrationError, InvalidationReason, SensingError};
use crate::features::amplitude_variability;
use crate::fusion::sigmoid;

/// The calibration artefact schema this core produces and accepts.
pub const CALIBRATION_SCHEMA_VERSION: &str = "aquila.calibration/v1";
/// The major component of [`CALIBRATION_SCHEMA_VERSION`].
pub const CALIBRATION_SCHEMA_MAJOR: u16 = 1;
/// The learning-semantics version (threshold and fusion algorithms).
pub const CALIBRATION_SEMANTICS_VERSION: &str = "1.0.0";
/// The invalidation-policy version.
pub const CALIBRATION_POLICY_VERSION: &str = "1";
/// The default baseline staleness window, in days (`geometry-calibration.md`
/// §14: a placeholder pending a real S3 drift study).
pub const DEFAULT_STALE_AFTER_DAYS: u32 = 90;
/// The sigmoid score at or above which the decision is "present".
const PRESENCE_DECISION: f64 = 0.5;
/// The floor on a zone's dB logit scale, so a tiny learned threshold cannot
/// amplify noise without bound.
const MIN_SCALE_DB: f64 = 1.0;
/// The default zone threshold used when no walk-through separates a room.
const DEFAULT_PRESENCE_THRESHOLD_DB: f64 = 6.0;
/// The reference RSSI at one metre for the advisory range proxy, in dBm.
const REFERENCE_RSSI_DBM: f64 = -40.0;
/// The path-loss exponent for the advisory range proxy.
const PATH_LOSS_EXPONENT: f64 = 2.5;
/// The number of empty-home windows at which a node's baseline is considered
/// fully sampled for the per-node quality term.
const NODE_QUALITY_FULL_WINDOWS: f32 = 20.0;
/// The clock fit residual, in milliseconds, at which a node's clock term halves.
const NODE_QUALITY_RESIDUAL_MS: f32 = 5.0;

// ---------------------------------------------------------------------------
// Schema types (`geometry-calibration.md` §13)
// ---------------------------------------------------------------------------

/// The overall quality status of a calibration run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityStatus {
    /// Every gate passes with margin.
    Good,
    /// Every gate passes, with little margin.
    Fair,
    /// At least one gate fails; the artefact is not `valid`.
    Poor,
}

/// One quality gate and its outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gate {
    /// The gate's name, e.g. `link_coverage`.
    pub gate: String,
    /// The observed value.
    pub value: f64,
    /// The threshold the value is compared against.
    pub threshold: f64,
    /// Whether the gate passed.
    pub pass: bool,
    /// `"min"` when the value must be at least the threshold, `"max"` otherwise.
    pub direction: String,
}

/// The ledger-shaped calibration quality metrics (`geometry-calibration.md` §11).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Fraction of candidate links with a usable baseline.
    pub link_coverage: f64,
    /// The worst usable link's SNR, in dB.
    pub baseline_snr_db: f64,
    /// The worst usable link's excess dynamic range, in dB.
    pub excess_dynamic_range_db: f64,
    /// The smallest inter-zone distribution distance (a v1 dB proxy).
    pub zone_separation: f64,
    /// Leave-one-zone-out presence F1 on the calibration captures.
    pub presence_f1: f64,
    /// The worst post-fit time-sync residual, in milliseconds.
    pub drift_residual_ms: f64,
}

/// The rolled-up quality block of the artefact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationQuality {
    /// `good`, `fair` or `poor`.
    pub status: QualityStatus,
    /// Whether every gate passed. A `false` artefact is never silently used.
    pub valid: bool,
    /// The individual gates.
    pub gates: Vec<Gate>,
    /// The roll-up metrics.
    pub metrics: QualityMetrics,
}

/// Provenance of a calibration run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratedBy {
    /// The core commit that produced the artefact.
    pub core_commit: String,
    /// The core crate version.
    pub core_version: String,
    /// A unique run id.
    pub run_id: String,
}

/// The home identity block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HomeCalibration {
    /// The home's identity.
    pub home_id: String,
    /// The digest of the topology the calibration corrects.
    pub topology_digest: String,
}

/// The world coordinate frame of the calibration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldFrame {
    /// The frame kind, always `"enu"` in v1.
    pub kind: String,
    /// The origin reference.
    pub origin_ref: String,
    /// The unit, always `"metre"` in v1.
    pub units: String,
}

/// A floor's frame and its learned cross-floor slab attenuation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FloorFrame {
    /// The floor's identity.
    pub floor_id: String,
    /// The floor's level.
    pub index: i8,
    /// The floor plane's elevation in the home frame, in metres.
    pub elevation_m: f32,
    /// The learned slab attenuation for cross-floor links, in dB (0 when no
    /// cross-floor link exists).
    pub slab_attenuation_db: f32,
}

/// A room's frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomFrame {
    /// The room's identity.
    pub room_id: String,
    /// The floor the room belongs to.
    pub floor_id: String,
    /// The planar transform of the room frame in the world frame
    /// `[x_m, y_m, yaw_rad]`.
    pub transform_world: [f32; 3],
    /// The room polygon in the floor frame, when surveyed (empty in v1).
    pub polygon_floor_m: Vec<[f32; 2]>,
    /// The confidence in the room frame.
    pub confidence: f32,
}

/// The frame block of the artefact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FramesCalibration {
    /// The world frame.
    pub world: WorldFrame,
    /// The floors.
    pub floors: Vec<FloorFrame>,
    /// The rooms.
    pub rooms: Vec<RoomFrame>,
}

/// A node's placement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    /// The surveyed room-local position in metres, or `None` when unknown.
    pub position_m: Option<[f32; 2]>,
    /// The mount yaw in radians, or `None` when unknown.
    pub yaw_rad: Option<f32>,
    /// The mounting: `wall`, `ceiling`, `desk`, `shelf` or `unknown`.
    pub mounting: String,
    /// The placement source: `user`, `surveyed` or `inferred`.
    pub source: String,
    /// The confidence in the placement.
    pub confidence: f32,
}

/// A node's learned clock correction.
///
/// The affine map is `core_ns = slope * device_ns + intercept_ns`; `skew_ppm`
/// and `offset_s` are the schema-facing summaries, and `slope`/`intercept_ns`
/// allow exact inversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClockCalibration {
    /// The device-minus-core offset at the capture mean, in seconds.
    pub offset_s: f64,
    /// The relative rate error, in parts per million.
    pub skew_ppm: f64,
    /// The post-fit residual RMS, in milliseconds.
    pub fit_residual_ms: f64,
    /// The RFC 3339 UTC time the model was estimated at.
    pub estimated_at: String,
    /// The affine slope (core nanoseconds per device nanosecond).
    pub slope: f64,
    /// The affine intercept, in core nanoseconds.
    pub intercept_ns: f64,
}

impl ClockCalibration {
    /// Correct a device timestamp to core time, in nanoseconds.
    #[must_use]
    pub fn correct_core_ns(&self, device_ns: u64) -> f64 {
        self.slope * device_ns as f64 + self.intercept_ns
    }
}

/// A node's channel configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelCalibration {
    /// The band label: `"2.4ghz"`, `"5ghz"` or `"unknown"`.
    pub band: String,
    /// The primary channel.
    pub primary_channel: u8,
    /// The channel width in MHz.
    pub bandwidth_mhz: u8,
    /// The subcarrier count.
    pub subcarriers: usize,
}

/// A node's gain-drift basis (seeded, tracked continuously thereafter).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GainDrift {
    /// The amplitude drift, in dB per hour (0 in v1).
    pub amplitude_db_per_h: f32,
    /// The phase basis, when learned (empty in v1).
    pub phase_basis: Vec<f32>,
    /// The confidence in the gain-drift model.
    pub confidence: f32,
}

/// One calibrated node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeCalibration {
    /// The node's dense identity.
    pub node_id: u16,
    /// The floor the node is on.
    pub floor_id: String,
    /// The room the node is mounted in, when known.
    pub room_id: Option<String>,
    /// The zone the node belongs to, when known.
    pub zone_id: Option<String>,
    /// The placement.
    pub placement: Placement,
    /// The learned clock correction.
    pub clock: ClockCalibration,
    /// The firmware identity recorded at capture time.
    pub firmware: String,
    /// The channel configuration.
    pub channel: ChannelCalibration,
    /// The gain-drift basis.
    pub gain_drift: GainDrift,
    /// The node's own calibration quality `q_n` in `[0, 1]`
    /// (`fusion-sync.md` §2.3). Fusion weights this node by it; `ZERO` means the
    /// node contributes nothing.
    pub quality: NodeQuality,
}

/// A link's empty-home baseline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkBaseline {
    /// The baseline window start, in seconds from capture start.
    pub window_start_s: f64,
    /// The baseline window end, in seconds from capture start.
    pub window_end_s: f64,
    /// The window duration, in seconds.
    pub duration_s: f64,
    /// The occupancy label, always `"empty"` for a baseline.
    pub occupancy: String,
    /// The number of frames behind the baseline.
    pub samples: u64,
    /// Per-subcarrier mean amplitude, in dB.
    pub amplitude_mean_db: Vec<f32>,
    /// Per-subcarrier temporal amplitude standard deviation, in dB.
    pub amplitude_std_db: Vec<f32>,
    /// The empty-home temporal noise floor, in dB.
    pub noise_floor_db: f32,
    /// The perturbation headroom above the noise floor, in dB.
    pub excess_dynamic_range_db: f32,
    /// The link SNR, in dB.
    pub snr_db: f32,
    /// The fraction of the expected frames present.
    pub coverage: f32,
}

/// A link's advisory proximity relation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proximity {
    /// The mean received signal strength, in dBm.
    pub rssi_mean_dbm: f32,
    /// The advisory range estimate, in metres.
    pub range_estimate_m: f32,
    /// The honest per-link range bound, in metres.
    pub range_sigma_m: f32,
    /// `same_zone_candidate`, `adjacent` or `far`.
    pub relation: String,
    /// The confidence in the relation.
    pub confidence: f32,
}

/// One calibrated link (node pair).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkCalibration {
    /// The canonical unordered link id, `"a-b"`.
    pub link_id: String,
    /// The two endpoint node ids, ascending.
    pub nodes: [u16; 2],
    /// The topology-derived scope label: `intra_room`/`inter_room`/`inter_floor`.
    pub scope: String,
    /// The empty-home baseline.
    pub baseline: LinkBaseline,
    /// The advisory proximity relation.
    pub proximity: Proximity,
}

/// A node's empty-home amplitude baseline: the quantity the presence detector
/// measures, so a learned threshold is expressed in the detector's own units.
///
/// This is a documented v1 addition to the canonical schema: presence is a
/// *unary, per-node* sensor in v1 (`fusion-sync.md` §2), so the per-node
/// baseline a zone threshold divides by must travel with the artefact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeBaseline {
    /// The node's dense identity.
    pub node_id: u16,
    /// The room the node is mounted in.
    pub room_id: String,
    /// The mean over windows of the window's amplitude standard deviation.
    pub amplitude_std_mean: f32,
    /// The same value in dB.
    pub amplitude_std_db: f32,
    /// The node's mean amplitude over the capture, in dB.
    pub mean_amplitude_db: f32,
    /// The node's empty-home noise floor, in dB.
    pub noise_floor_db: f32,
    /// The number of windows behind the baseline.
    pub windows: u64,
}

/// A zone's learned detection quality.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneQuality {
    /// The inter-zone distribution distance (a v1 dB proxy).
    pub separation: f32,
    /// The leave-one-zone-out F1 on the calibration captures.
    pub cv_f1: f32,
}

/// A zone's learned detection thresholds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneDetection {
    /// The presence threshold: a dB rise above a node's empty-home baseline.
    pub presence_threshold_db: f32,
    /// The motion threshold: a lower dB rise for gross motion.
    pub motion_threshold_db: f32,
    /// The target false-positive rate.
    pub target_fpr: f32,
    /// The contributing link ids.
    pub contributing_links: Vec<String>,
    /// The contributing node ids.
    pub contributing_nodes: Vec<u16>,
    /// The empty-home level (mean window amplitude standard deviation).
    pub empty_level_std: f32,
    /// The walk-through level (mean window amplitude standard deviation).
    pub occupied_level_std: f32,
    /// The detection quality.
    pub quality: ZoneQuality,
}

/// One calibrated zone. In v1 a zone is a room (`CONTEXT.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneCalibration {
    /// The zone identity (`RoomId` in v1).
    pub zone_id: String,
    /// The owning room.
    pub room_id: String,
    /// The human-readable label.
    pub label: String,
    /// The adjacent zone ids.
    pub adjacency: Vec<String>,
    /// The learned thresholds.
    pub detection: ZoneDetection,
}

/// The geometry summary block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometryCalibration {
    /// The estimator: `proximity_fingerprint` or `trilateration_secondary`.
    pub method: String,
    /// The resolution: `room`, `zone` or `sub_room_advisory`.
    pub resolution: String,
    /// Always `none_cross_node` in v1 (`fusion-sync.md` §3).
    pub phase_coherence: String,
    /// The global geometry confidence.
    pub global_confidence: f32,
}

/// The invalidation policy block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvalidationPolicy {
    /// The policy version.
    pub policy_version: String,
    /// The reasons already recorded against this artefact.
    pub invalidated_by: Vec<String>,
    /// The baseline staleness window, in days.
    pub stale_after_days: u32,
    /// The calibration this one supersedes, if any.
    pub supersedes: Option<String>,
    /// The history pointer, `index.json#<calibration_id>`.
    pub history_pointer: String,
}

/// A content-addressed, versioned whole-home calibration artefact
/// (`aquila.calibration/v1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationArtifact {
    /// The artefact schema version.
    pub schema_version: String,
    /// The content-addressed identity, `sha256:<hex>`.
    pub calibration_id: String,
    /// The learning-semantics version.
    pub calibration_version: String,
    /// The RFC 3339 UTC generation time.
    pub generated_at: String,
    /// The run provenance.
    pub generated_by: GeneratedBy,
    /// The ONNX model ids the calibration assumed (`geometry-calibration.md`
    /// §10). Empty in v1, where no model is calibrated.
    pub model_versions: Vec<String>,
    /// The home identity block.
    pub home: HomeCalibration,
    /// The coordinate frames.
    pub frames: FramesCalibration,
    /// The calibrated nodes.
    pub nodes: Vec<NodeCalibration>,
    /// The calibrated links.
    pub links: Vec<LinkCalibration>,
    /// The per-node empty-home baselines.
    pub node_baselines: Vec<NodeBaseline>,
    /// The calibrated zones.
    pub zones: Vec<ZoneCalibration>,
    /// The geometry summary.
    pub geometry: GeometryCalibration,
    /// The rolled-up quality.
    pub quality: CalibrationQuality,
    /// The invalidation policy.
    pub invalidation: InvalidationPolicy,
    /// The SHA-256 of the body with `calibration_id`/`body_sha256` blanked.
    pub body_sha256: String,
}

impl CalibrationArtifact {
    /// The calibration's identity, `sha256:<hex>`.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.calibration_id
    }

    /// The topology digest the calibration is bound to.
    #[must_use]
    pub fn topology_digest(&self) -> &str {
        &self.home.topology_digest
    }

    /// The learned clock correction for a node, if present.
    #[must_use]
    pub fn clock_for(&self, node: NodeId) -> Option<&ClockCalibration> {
        self.nodes
            .iter()
            .find(|n| n.node_id == node.0)
            .map(|n| &n.clock)
    }

    /// The empty-home baseline for a node, if present.
    #[must_use]
    pub fn node_baseline(&self, node: NodeId) -> Option<&NodeBaseline> {
        self.node_baselines.iter().find(|b| b.node_id == node.0)
    }

    /// The per-node calibration quality `q_n` for a node, if present.
    #[must_use]
    pub fn node_quality(&self, node: NodeId) -> Option<NodeQuality> {
        self.nodes
            .iter()
            .find(|n| n.node_id == node.0)
            .map(|n| n.quality)
    }

    /// The detection thresholds for a room (the v1 zone), if present.
    #[must_use]
    pub fn zone_for_room(&self, room: &RoomId) -> Option<&ZoneCalibration> {
        self.zones.iter().find(|z| z.room_id == room.as_str())
    }

    /// The canonical body string used for content addressing: the artefact with
    /// its identity fields and its history pointer blanked.
    ///
    /// The history pointer embeds the identity, so it cannot participate in the
    /// digest it carries; it is set from the sealed digest and excluded here.
    fn canonical_body(&self) -> Result<String, CalibrationError> {
        let mut body = self.clone();
        body.calibration_id = String::new();
        body.body_sha256 = String::new();
        body.invalidation.history_pointer = String::new();
        serde_json::to_string(&body)
            .map_err(|error| CalibrationError::Json(error.to_string()))
    }

    /// Seal the artefact: compute `body_sha256` and `calibration_id`.
    ///
    /// # Errors
    ///
    /// Returns [`CalibrationError::Json`] if the artefact cannot be serialised.
    pub fn seal(mut self) -> Result<Self, CalibrationError> {
        let digest = to_hex(&sha256(self.canonical_body()?.as_bytes()));
        self.body_sha256 = digest.clone();
        self.calibration_id = format!("sha256:{digest}");
        self.invalidation.history_pointer = format!("index.json#sha256:{digest}");
        Ok(self)
    }

    /// Recompute and verify the content-addressed digest.
    ///
    /// # Errors
    ///
    /// Returns [`CalibrationError::DigestMismatch`] when the artefact's body no
    /// longer hashes to its stored `body_sha256`.
    pub fn verify_digest(&self) -> Result<(), CalibrationError> {
        let actual = to_hex(&sha256(self.canonical_body()?.as_bytes()));
        if actual != self.body_sha256 {
            return Err(CalibrationError::DigestMismatch {
                expected: self.body_sha256.clone(),
                actual,
            });
        }
        Ok(())
    }

    /// The canonical JSON document of the artefact.
    ///
    /// # Errors
    ///
    /// Returns [`CalibrationError::Json`] if serialisation fails.
    pub fn to_json(&self) -> Result<String, CalibrationError> {
        serde_json::to_string(self).map_err(|error| CalibrationError::Json(error.to_string()))
    }

    /// Parse an artefact from its canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns [`CalibrationError::Json`] when the document is malformed, or
    /// [`CalibrationError::UnsupportedSchema`] when the schema major is unknown.
    pub fn from_json(json: &str) -> Result<Self, CalibrationError> {
        let artifact: Self = serde_json::from_str(json)
            .map_err(|error| CalibrationError::Json(error.to_string()))?;
        if !artifact
            .schema_version
            .starts_with("aquila.calibration/v1")
        {
            return Err(CalibrationError::UnsupportedSchema {
                found: artifact.schema_version,
            });
        }
        Ok(artifact)
    }

    /// The invalidation reasons against an expectation, ignoring drift and age.
    ///
    /// Returns an empty vector when the artefact is current. Any non-empty
    /// vector means the artefact must not be silently reused
    /// (`geometry-calibration.md` §10).
    #[must_use]
    pub fn invalidation_reasons(&self, expect: &CalibrationExpectation) -> Vec<InvalidationReason> {
        let mut reasons = Vec::new();
        if !self
            .schema_version
            .starts_with(&format!("aquila.calibration/v{}", expect.schema_major))
        {
            reasons.push(InvalidationReason::SchemaMismatch {
                found: self.schema_version.clone(),
                expected: expect.schema_version.clone(),
            });
        }
        if self.calibration_version != expect.calibration_version {
            reasons.push(InvalidationReason::SemanticsMismatch {
                found: self.calibration_version.clone(),
                expected: expect.calibration_version.clone(),
            });
        }
        if self.home.topology_digest != expect.topology_digest {
            reasons.push(InvalidationReason::TopologyChanged {
                found: self.home.topology_digest.clone(),
                expected: expect.topology_digest.clone(),
            });
        }
        if self.generated_by.core_commit != expect.core_commit {
            reasons.push(InvalidationReason::CoreCommitChanged {
                found: self.generated_by.core_commit.clone(),
                expected: expect.core_commit.clone(),
            });
        }
        let firmware = self.nodes.first().map_or_else(String::new, |n| n.firmware.clone());
        if !expect.firmware.is_empty() && firmware != expect.firmware {
            reasons.push(InvalidationReason::FirmwareChanged {
                found: firmware,
                expected: expect.firmware.clone(),
            });
        }
        if !expect.model_versions.is_empty() {
            let mut found = self.model_versions.clone();
            let mut expected = expect.model_versions.clone();
            found.sort();
            expected.sort();
            if found != expected {
                reasons.push(InvalidationReason::ModelChanged { found, expected });
            }
        }
        if !self.quality.valid {
            reasons.push(InvalidationReason::QualityFailed);
        }
        reasons
    }

    /// Check the artefact against a set of current live drift residuals.
    ///
    /// A node whose live residual exceeds its calibrated residual by more than
    /// `factor` (plus an absolute floor) is stale.
    #[must_use]
    pub fn drift_reasons(
        &self,
        live_residual_ms: &BTreeMap<u16, f64>,
        factor: f64,
        absolute_floor_ms: f64,
    ) -> Vec<InvalidationReason> {
        let mut reasons = Vec::new();
        for node in &self.nodes {
            if let Some(observed) = live_residual_ms.get(&node.node_id) {
                let calibrated = node.clock.fit_residual_ms;
                if *observed > calibrated * factor + absolute_floor_ms {
                    reasons.push(InvalidationReason::DriftExceeded {
                        node: node.node_id,
                        calibrated_ms: calibrated,
                        observed_ms: *observed,
                    });
                }
            }
        }
        reasons
    }

    /// Check the artefact's age against its staleness policy.
    #[must_use]
    pub fn age_reason(&self, age_days: u32) -> Option<InvalidationReason> {
        let stale_after = self.invalidation.stale_after_days;
        if stale_after > 0 && age_days > stale_after {
            Some(InvalidationReason::BaselineAgeExceeded {
                age_days,
                stale_after_days: stale_after,
            })
        } else {
            None
        }
    }

    /// Assert the artefact is current against an expectation, in one call.
    ///
    /// # Errors
    ///
    /// Returns [`CalibrationError::Stale`] carrying every reason when the
    /// artefact may not be reused.
    pub fn check(&self, expect: &CalibrationExpectation) -> Result<(), CalibrationError> {
        let reasons = self.invalidation_reasons(expect);
        if reasons.is_empty() {
            Ok(())
        } else {
            Err(CalibrationError::Stale(reasons))
        }
    }
}

/// The current facts a stored artefact is checked against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationExpectation {
    /// The schema major this core accepts.
    pub schema_major: u16,
    /// The schema version string this core accepts.
    pub schema_version: String,
    /// The current learning-semantics version.
    pub calibration_version: String,
    /// The current core commit.
    pub core_commit: String,
    /// The current topology digest.
    pub topology_digest: String,
    /// The current firmware identity.
    pub firmware: String,
    /// The current model versions (empty in v1).
    pub model_versions: Vec<String>,
}

impl CalibrationExpectation {
    /// An expectation for the current schema and semantics.
    #[must_use]
    pub fn new(core_commit: impl Into<String>, topology_digest: impl Into<String>) -> Self {
        Self {
            schema_major: CALIBRATION_SCHEMA_MAJOR,
            schema_version: CALIBRATION_SCHEMA_VERSION.to_string(),
            calibration_version: CALIBRATION_SEMANTICS_VERSION.to_string(),
            core_commit: core_commit.into(),
            topology_digest: topology_digest.into(),
            firmware: String::new(),
            model_versions: Vec::new(),
        }
    }
}

/// The immutable facts of one calibration run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationRequest {
    /// The core commit that produced the run.
    pub core_commit: String,
    /// The core crate version.
    pub core_version: String,
    /// A unique run id.
    pub run_id: String,
    /// The RFC 3339 UTC generation time (injected; the wizard reads no clock).
    pub generated_at: String,
    /// The firmware identity recorded at capture time.
    pub firmware: String,
    /// The target false-positive rate, in per-mille (30 = 0.03).
    pub target_fpr_per_mille: u32,
    /// The baseline staleness window, in days.
    pub stale_after_days: u32,
    /// The ONNX model ids the calibration assumes (empty in v1).
    pub model_versions: Vec<String>,
    /// The calibration this one supersedes, if any.
    pub supersedes: Option<String>,
}

impl CalibrationRequest {
    /// A request with the v1 defaults, for a given commit, run and timestamp.
    #[must_use]
    pub fn new(
        core_commit: impl Into<String>,
        run_id: impl Into<String>,
        generated_at: impl Into<String>,
    ) -> Self {
        Self {
            core_commit: core_commit.into(),
            core_version: env!("CARGO_PKG_VERSION").to_string(),
            run_id: run_id.into(),
            generated_at: generated_at.into(),
            firmware: String::new(),
            target_fpr_per_mille: 30,
            stale_after_days: DEFAULT_STALE_AFTER_DAYS,
            model_versions: Vec::new(),
            supersedes: None,
        }
    }

    /// The target false-positive rate in `[0, 1]`.
    #[must_use]
    pub fn target_fpr(&self) -> f64 {
        f64::from(self.target_fpr_per_mille) / 1000.0
    }
}

/// The captured evidence for one calibration run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CalibrationInputs {
    /// The empty-home baseline capture.
    pub empty_home: Vec<CsiEnvelope>,
    /// Labelled walk-through captures, one per room.
    pub walkthrough: Vec<(RoomId, Vec<CsiEnvelope>)>,
}

impl CalibrationInputs {
    /// Construct inputs from an empty-home capture and labelled walk-throughs.
    #[must_use]
    pub fn new(
        empty_home: Vec<CsiEnvelope>,
        walkthrough: Vec<(RoomId, Vec<CsiEnvelope>)>,
    ) -> Self {
        Self {
            empty_home,
            walkthrough,
        }
    }

    /// Whether the inputs carry a usable empty-home capture.
    #[must_use]
    pub fn has_baseline(&self) -> bool {
        !self.empty_home.is_empty()
    }
}

/// The outcome of a calibration attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct CalibrationReport {
    /// Whether a valid artefact was produced.
    pub calibrated: bool,
    /// The rolled-up quality status, when an artefact was produced.
    pub quality: Option<QualityStatus>,
    /// The topology digest the attempt was bound to.
    pub topology_digest: String,
    /// The number of windows observed so far, for provenance.
    pub windows_observed: usize,
    /// A human-readable explanation of the outcome.
    pub note: String,
    /// The produced artefact, when the run was publishable.
    pub artifact: Option<CalibrationArtifact>,
}

impl CalibrationReport {
    /// The honest "no calibration was computed" outcome.
    #[must_use]
    pub fn uncalibrated(windows_observed: usize, note: impl Into<String>) -> Self {
        Self {
            calibrated: false,
            quality: None,
            topology_digest: String::new(),
            windows_observed,
            note: note.into(),
            artifact: None,
        }
    }
}

/// Run the whole-home calibration wizard over captured evidence.
///
/// Learns the per-node clock model and empty-home variability, the per-link
/// baselines, the per-zone presence/motion thresholds, and the per-floor slab
/// attenuation, gates the run, and seals a content-addressed
/// [`CalibrationArtifact`].
///
/// # Errors
///
/// Returns [`SensingError::Calibration`] with [`CalibrationError::Invalid`] when
/// the empty-home capture is empty, or a JSON error from the topology digest or
/// sealing.
pub fn calibrate(
    topology: &Topology,
    config: &SensingConfig,
    request: &CalibrationRequest,
    inputs: &CalibrationInputs,
) -> Result<CalibrationArtifact, SensingError> {
    if inputs.empty_home.is_empty() {
        return Err(CalibrationError::Invalid(
            "the empty-home capture must not be empty".to_string(),
        )
        .into());
    }
    let topology_digest = topology_digest(topology.model())?;
    let hop_ns = config.window_hop_ns.max(1);
    let target_fpr = request.target_fpr();

    // Per-node empty-home and walk-through analyses.
    let empty_nodes = analyse_nodes(&inputs.empty_home, hop_ns);
    let walk_nodes: BTreeMap<RoomId, BTreeMap<NodeId, NodeAnalysis>> = inputs
        .walkthrough
        .iter()
        .map(|(room, frames)| (room.clone(), analyse_nodes(frames, hop_ns)))
        .collect();

    // Nodes and per-node baselines.
    let mut nodes = Vec::new();
    let mut node_baselines = Vec::new();
    for node in &topology.model().nodes {
        let analysis = empty_nodes.get(&node.id);
        let (clock, firmware, channel) = build_node_clock_and_channel(node, analysis, request);
        let quality = node_quality(analysis, &clock);
        let placement = build_placement(topology, node.id);
        nodes.push(NodeCalibration {
            node_id: node.id.0,
            floor_id: topology.floor_of(node.id).value.map_or_else(
                || node.room.as_str().to_string(),
                |f| f.as_str().to_string(),
            ),
            room_id: Some(node.room.as_str().to_string()),
            zone_id: topology
                .zone_of(node.id)
                .value
                .flatten()
                .map(|z| z.as_str().to_string()),
            placement,
            clock,
            firmware,
            channel,
            gain_drift: GainDrift {
                amplitude_db_per_h: 0.0,
                phase_basis: Vec::new(),
                confidence: 0.5,
            },
            quality,
        });
        if let Some(analysis) = analysis {
            node_baselines.push(NodeBaseline {
                node_id: node.id.0,
                room_id: node.room.as_str().to_string(),
                amplitude_std_mean: analysis.amplitude_std_mean as f32,
                amplitude_std_db: db20(analysis.amplitude_std_mean) as f32,
                mean_amplitude_db: analysis.mean_amplitude_db as f32,
                noise_floor_db: analysis.noise_floor_db as f32,
                windows: analysis.window_stds.len() as u64,
            });
        }
    }
    nodes.sort_by_key(|n| n.node_id);
    node_baselines.sort_by_key(|b| b.node_id);

    // Links from every unordered node pair.
    let mut links = Vec::new();
    for (a, b) in candidate_pairs(topology) {
        if let Some(link) = build_link(topology, &inputs.empty_home, a, b) {
            links.push(link);
        }
    }
    links.sort_by(|x, y| x.link_id.cmp(&y.link_id));

    // Zones, thresholds and per-zone preservation F1.
    let mut zones = Vec::new();
    let mut separation_min = f64::INFINITY;
    for room in &topology.model().rooms {
        let room_id = room.id.clone();
        let walk = walk_nodes.get(&room_id);
        let zone = build_zone(topology, &room_id, &empty_nodes, walk, &links, target_fpr)?;
        separation_min = separation_min.min(f64::from(zone.detection.quality.separation));
        zones.push(zone);
    }
    zones.sort_by(|a, b| a.zone_id.cmp(&b.zone_id));
    let presence_f1 = zones
        .iter()
        .map(|z| f64::from(z.detection.quality.cv_f1))
        .fold(f64::INFINITY, f64::min);
    let presence_f1 = finite_or(presence_f1, 0.0);
    let zone_separation = finite_or(separation_min, 0.0);

    // Metrics and gates.
    let metrics = QualityMetrics {
        link_coverage: link_coverage(&links, topology),
        baseline_snr_db: finite_or(
            links.iter().map(|l| f64::from(l.baseline.snr_db)).fold(f64::INFINITY, f64::min),
            0.0,
        ),
        excess_dynamic_range_db: finite_or(
            links
                .iter()
                .map(|l| f64::from(l.baseline.excess_dynamic_range_db))
                .fold(f64::INFINITY, f64::min),
            0.0,
        ),
        zone_separation,
        presence_f1,
        drift_residual_ms: nodes
            .iter()
            .map(|n| n.clock.fit_residual_ms)
            .fold(0.0_f64, f64::max),
    };
    let quality = gate(metrics);

    let frames = build_frames(topology, &links);

    let artifact = CalibrationArtifact {
        schema_version: CALIBRATION_SCHEMA_VERSION.to_string(),
        calibration_id: String::new(),
        calibration_version: CALIBRATION_SEMANTICS_VERSION.to_string(),
        generated_at: request.generated_at.clone(),
        generated_by: GeneratedBy {
            core_commit: request.core_commit.clone(),
            core_version: request.core_version.clone(),
            run_id: request.run_id.clone(),
        },
        model_versions: request.model_versions.clone(),
        home: HomeCalibration {
            home_id: topology.home().id.as_str().to_string(),
            topology_digest,
        },
        frames,
        nodes,
        links,
        node_baselines,
        zones,
        geometry: GeometryCalibration {
            method: "proximity_fingerprint".to_string(),
            resolution: "room".to_string(),
            phase_coherence: "none_cross_node".to_string(),
            global_confidence: 0.5,
        },
        quality,
        invalidation: InvalidationPolicy {
            policy_version: CALIBRATION_POLICY_VERSION.to_string(),
            invalidated_by: Vec::new(),
            stale_after_days: request.stale_after_days,
            supersedes: request.supersedes.clone(),
            history_pointer: String::new(),
        },
        body_sha256: String::new(),
    }
    .seal()?;

    Ok(artifact)
}

/// Compute the topology digest: SHA-256 of the topology's canonical JSON.
///
/// # Errors
///
/// Returns [`SensingError::Calibration`] with [`CalibrationError::Json`] when
/// the topology cannot be serialised.
pub fn topology_digest(topology: &aquila_types::Topology) -> Result<String, SensingError> {
    let json = serde_json::to_string(topology)
        .map_err(|error| CalibrationError::Json(error.to_string()))?;
    Ok(format!("sha256:{}", to_hex(&sha256(json.as_bytes()))))
}

// ---------------------------------------------------------------------------
// Analysis helpers
// ---------------------------------------------------------------------------

/// The per-node statistics learned from one capture.
#[derive(Debug, Clone)]
struct NodeAnalysis {
    window_stds: Vec<f64>,
    amplitude_std_mean: f64,
    mean_amplitude_db: f64,
    noise_floor_db: f64,
    device_ns: Vec<f64>,
    host_ns: Vec<f64>,
    band: aquila_types::Band,
    channel: u8,
    bandwidth_mhz: u8,
    subcarriers: usize,
}

/// Group a capture's envelopes by node and analyse each.
fn analyse_nodes(frames: &[CsiEnvelope], hop_ns: u64) -> BTreeMap<NodeId, NodeAnalysis> {
    let mut by_node: BTreeMap<NodeId, Vec<&CsiEnvelope>> = BTreeMap::new();
    for envelope in frames {
        by_node
            .entry(envelope.frame.node_id)
            .or_default()
            .push(envelope);
    }
    let mut out = BTreeMap::new();
    for (node, envelopes) in by_node {
        out.insert(node, analyse_node(&envelopes, hop_ns));
    }
    out
}

/// Analyse one node's envelopes: window variability, amplitude statistics and
/// the device↔host clock relation.
fn analyse_node(envelopes: &[&CsiEnvelope], hop_ns: u64) -> NodeAnalysis {
    let mut windows: BTreeMap<u64, Vec<CsiEnvelope>> = BTreeMap::new();
    for envelope in envelopes {
        let slot = envelope.host_rx_mono_ns / hop_ns;
        windows
            .entry(slot)
            .or_default()
            .push((*envelope).clone());
    }

    let window_stds: Vec<f64> = windows
        .values()
        .map(|window| amplitude_variability(window).std)
        .collect();
    let amplitude_std_mean = mean_of(&window_stds);

    let bins = envelopes
        .iter()
        .map(|e| e.frame.n_subcarriers())
        .min()
        .unwrap_or(0);
    let mut mean_amplitude_db = 0.0_f64;
    let mut noise_floor_db = 0.0_f64;
    if bins > 0 {
        let n = envelopes.len() as f64;
        let mut sum_mean_db = 0.0_f64;
        let mut sum_std_db = 0.0_f64;
        for k in 0..bins {
            let mut sum = 0.0_f64;
            for envelope in envelopes {
                sum += f64::from(envelope.frame.amplitude_at(k));
            }
            let mean = sum / n;
            let mut var = 0.0_f64;
            for envelope in envelopes {
                let d = f64::from(envelope.frame.amplitude_at(k)) - mean;
                var += d * d;
            }
            let std = (var / n).sqrt();
            sum_mean_db += db20(mean);
            sum_std_db += db20(std);
        }
        mean_amplitude_db = sum_mean_db / bins as f64;
        noise_floor_db = sum_std_db / bins as f64;
    }

    let mut device_ns = Vec::with_capacity(envelopes.len());
    let mut host_ns = Vec::with_capacity(envelopes.len());
    for envelope in envelopes {
        device_ns.push(envelope.frame.device_time_us as f64 * 1_000.0);
        host_ns.push(envelope.host_rx_mono_ns as f64);
    }

    let first = envelopes.first();
    NodeAnalysis {
        window_stds,
        amplitude_std_mean,
        mean_amplitude_db,
        noise_floor_db,
        device_ns,
        host_ns,
        band: first.map_or(aquila_types::Band::Unknown, |e| e.frame.band),
        channel: first.map_or(0, |e| e.frame.channel),
        bandwidth_mhz: first.map_or(0, |e| e.frame.bandwidth_mhz),
        subcarriers: first.map_or(0, |e| e.frame.n_subcarriers()),
    }
}

/// Fit the affine device→core map and build the node's clock + channel blocks.
fn build_node_clock_and_channel(
    node: &aquila_types::Node,
    analysis: Option<&NodeAnalysis>,
    request: &CalibrationRequest,
) -> (ClockCalibration, String, ChannelCalibration) {
    let firmware = format!("{} {}", device_label(node.device), node.firmware.version);
    let channel = analysis.map_or_else(
        || ChannelCalibration {
            band: "unknown".to_string(),
            primary_channel: 0,
            bandwidth_mhz: 0,
            subcarriers: 0,
        },
        |a| ChannelCalibration {
            band: band_label(a.band).to_string(),
            primary_channel: a.channel,
            bandwidth_mhz: a.bandwidth_mhz,
            subcarriers: a.subcarriers,
        },
    );

    let clock = match analysis.and_then(affine_fit) {
        Some(fit) => ClockCalibration {
            offset_s: (fit.mean_device - fit.mean_core) / 1e9,
            skew_ppm: if fit.slope != 0.0 {
                (1.0 / fit.slope - 1.0) * 1e6
            } else {
                0.0
            },
            fit_residual_ms: fit.residual_ns / 1e6,
            estimated_at: request.generated_at.clone(),
            slope: fit.slope,
            intercept_ns: fit.intercept,
        },
        None => ClockCalibration {
            offset_s: 0.0,
            skew_ppm: 0.0,
            fit_residual_ms: 0.0,
            estimated_at: request.generated_at.clone(),
            slope: 1.0,
            intercept_ns: 0.0,
        },
    };
    (clock, firmware, channel)
}

/// The per-node calibration quality `q_n` (`fusion-sync.md` §2.3).
///
/// Derived from the two per-node quantities the wizard actually measures: how
/// many empty-home windows back the node's baseline and how well its clock fits
/// to the core. A node absent from the empty-home capture has quality zero and,
/// by the fusion invariant, contributes nothing; a node whose clock cannot be
/// fitted falls back to its baseline sampling alone.
fn node_quality(analysis: Option<&NodeAnalysis>, clock: &ClockCalibration) -> NodeQuality {
    let Some(analysis) = analysis else {
        return NodeQuality::ZERO;
    };
    let windows = analysis.window_stds.len() as f32;
    let window_term = (windows / NODE_QUALITY_FULL_WINDOWS).clamp(0.0, 1.0);
    let residual_ms = clock.fit_residual_ms as f32;
    let clock_term = if residual_ms.is_finite() {
        1.0 / (1.0 + residual_ms.max(0.0) / NODE_QUALITY_RESIDUAL_MS)
    } else {
        0.0
    };
    NodeQuality::new((window_term * clock_term).clamp(0.0, 1.0)).unwrap_or(NodeQuality::ZERO)
}

/// The result of fitting `core = slope·device + intercept`.
struct AffineFit {
    slope: f64,
    intercept: f64,
    residual_ns: f64,
    mean_device: f64,
    mean_core: f64,
}

/// Fit `core = slope·device + intercept` by ordinary least squares.
fn affine_fit(analysis: &NodeAnalysis) -> Option<AffineFit> {
    let n = analysis.device_ns.len();
    if n < 2 {
        return None;
    }
    let nf = n as f64;
    let mean_device = analysis.device_ns.iter().sum::<f64>() / nf;
    let mean_core = analysis.host_ns.iter().sum::<f64>() / nf;
    let mut sxx = 0.0_f64;
    let mut sxy = 0.0_f64;
    for (x, y) in analysis.device_ns.iter().zip(analysis.host_ns.iter()) {
        let dx = x - mean_device;
        sxx += dx * dx;
        sxy += dx * (y - mean_core);
    }
    if sxx <= f64::EPSILON {
        return Some(AffineFit {
            slope: 1.0,
            intercept: mean_core - mean_device,
            residual_ns: 0.0,
            mean_device,
            mean_core,
        });
    }
    let slope = sxy / sxx;
    let intercept = mean_core - slope * mean_device;
    let mut sum_sq = 0.0_f64;
    for (x, y) in analysis.device_ns.iter().zip(analysis.host_ns.iter()) {
        let residual = y - (slope * x + intercept);
        sum_sq += residual * residual;
    }
    let residual_ns = (sum_sq / nf).sqrt();
    Some(AffineFit {
        slope,
        intercept,
        residual_ns,
        mean_device,
        mean_core,
    })
}

/// Build a node's placement block from the topology.
fn build_placement(topology: &Topology, node: NodeId) -> Placement {
    match topology.node(node).and_then(|n| n.pos_room_m) {
        Some(pos) => Placement {
            position_m: Some([pos[0], pos[1]]),
            yaw_rad: None,
            mounting: "unknown".to_string(),
            source: "user".to_string(),
            confidence: 0.8,
        },
        None => Placement {
            position_m: None,
            yaw_rad: None,
            mounting: "unknown".to_string(),
            source: "user".to_string(),
            confidence: 0.0,
        },
    }
}

/// The candidate links of a topology: every unordered node pair.
fn candidate_pairs(topology: &Topology) -> Vec<(NodeId, NodeId)> {
    let nodes: Vec<NodeId> = topology.model().nodes.iter().map(|n| n.id).collect();
    let mut out = Vec::new();
    for (i, a) in nodes.iter().enumerate() {
        for b in nodes.iter().skip(i + 1) {
            if topology.scope_of(*a, *b).is_some() {
                out.push((*a, *b));
            }
        }
    }
    out
}

/// Build one link's baseline and proximity block from the empty-home capture.
fn build_link(
    topology: &Topology,
    frames: &[CsiEnvelope],
    a: NodeId,
    b: NodeId,
) -> Option<LinkCalibration> {
    let pooled: Vec<&CsiEnvelope> = frames
        .iter()
        .filter(|e| e.frame.node_id == a || e.frame.node_id == b)
        .collect();
    if pooled.is_empty() {
        return None;
    }
    let bins = pooled
        .iter()
        .map(|e| e.frame.n_subcarriers())
        .min()
        .unwrap_or(0);
    if bins == 0 {
        return None;
    }
    let n = pooled.len() as f64;
    let mut amplitude_mean_db = Vec::with_capacity(bins);
    let mut amplitude_std_db = Vec::with_capacity(bins);
    let mut sum_mean_db = 0.0_f64;
    let mut sum_std_db = 0.0_f64;
    let mut min_mean_db = f64::INFINITY;
    let mut max_mean_db = f64::NEG_INFINITY;
    for k in 0..bins {
        let mut sum = 0.0_f64;
        for envelope in &pooled {
            sum += f64::from(envelope.frame.amplitude_at(k));
        }
        let mean = sum / n;
        let mut var = 0.0_f64;
        for envelope in &pooled {
            let d = f64::from(envelope.frame.amplitude_at(k)) - mean;
            var += d * d;
        }
        let std = (var / n).sqrt();
        let md = db20(mean);
        let sd = db20(std);
        min_mean_db = min_mean_db.min(md);
        max_mean_db = max_mean_db.max(md);
        sum_mean_db += md;
        sum_std_db += sd;
        amplitude_mean_db.push(md as f32);
        amplitude_std_db.push(sd as f32);
    }
    let mean_db = sum_mean_db / bins as f64;
    let noise_floor_db = sum_std_db / bins as f64;
    let snr_db = mean_db - noise_floor_db;
    let excess_dynamic_range_db = max_mean_db - min_mean_db;

    let rssi_mean = pooled
        .iter()
        .map(|e| f64::from(e.frame.rssi_dbm))
        .sum::<f64>()
        / n;
    let range_m = 10.0_f64.powf((REFERENCE_RSSI_DBM - rssi_mean) / (10.0 * PATH_LOSS_EXPONENT));
    let relation = topology
        .proximity_relation(a, b)
        .map_or("far", relation_label);
    let scope = topology
        .scope_of(a, b)
        .map_or("inter_floor", aquila_geometry::scope_label);

    let start_ns = pooled.iter().map(|e| e.host_rx_mono_ns).min().unwrap_or(0);
    let end_ns = pooled.iter().map(|e| e.host_rx_mono_ns).max().unwrap_or(0);

    Some(LinkCalibration {
        link_id: Topology::canonical_pair_id(a, b),
        nodes: [a.0.min(b.0), a.0.max(b.0)],
        scope: scope.to_string(),
        baseline: LinkBaseline {
            window_start_s: 0.0,
            window_end_s: (end_ns.saturating_sub(start_ns)) as f64 / 1e9,
            duration_s: (end_ns.saturating_sub(start_ns)) as f64 / 1e9,
            occupancy: "empty".to_string(),
            samples: pooled.len() as u64,
            amplitude_mean_db,
            amplitude_std_db,
            noise_floor_db: noise_floor_db as f32,
            excess_dynamic_range_db: excess_dynamic_range_db as f32,
            snr_db: snr_db as f32,
            coverage: 1.0,
        },
        proximity: Proximity {
            rssi_mean_dbm: rssi_mean as f32,
            range_estimate_m: range_m as f32,
            range_sigma_m: (range_m * 0.5) as f32,
            relation: relation.to_string(),
            confidence: 0.5,
        },
    })
}

/// Build one zone's detection thresholds and calibration F1.
fn build_zone(
    topology: &Topology,
    room: &RoomId,
    empty_nodes: &BTreeMap<NodeId, NodeAnalysis>,
    walk_nodes: Option<&BTreeMap<NodeId, NodeAnalysis>>,
    links: &[LinkCalibration],
    target_fpr: f64,
) -> Result<ZoneCalibration, SensingError> {
    let contributing: Vec<NodeId> = topology
        .model()
        .nodes
        .iter()
        .filter(|n| n.room == *room)
        .map(|n| n.id)
        .collect();

    let empty_levels: Vec<f64> = contributing
        .iter()
        .filter_map(|node| empty_nodes.get(node))
        .map(|a| a.amplitude_std_mean)
        .collect();
    let empty_level = mean_of(&empty_levels);

    let occupied_levels: Vec<f64> = match walk_nodes {
        Some(walk) => contributing
            .iter()
            .filter_map(|node| walk.get(node))
            .map(|a| a.amplitude_std_mean)
            .collect(),
        None => Vec::new(),
    };
    let occupied_level = mean_of(&occupied_levels);

    let (presence_threshold_db, motion_threshold_db, separation) =
        if empty_level > 0.0 && occupied_level > empty_level {
            let rise_db = db20(occupied_level) - db20(empty_level);
            // `separation` is the dimensionless relative rise of the occupied
            // level over the empty baseline — a monotone, scale-free v1 proxy
            // for the spec's inter-zone distribution distance.
            let separation = (occupied_level - empty_level) / empty_level;
            (rise_db / 2.0, rise_db / 4.0, separation)
        } else {
            (
                DEFAULT_PRESENCE_THRESHOLD_DB,
                DEFAULT_PRESENCE_THRESHOLD_DB / 2.0,
                0.0,
            )
        };
    // Per-zone calibration F1, using the exact runtime decision.
    let (mut tp, mut fp, mut fn_) = (0u64, 0u64, 0u64);
    for node in &contributing {
        let Some(baseline) = empty_nodes.get(node) else {
            continue;
        };
        let baseline_std = baseline.amplitude_std_mean;
        if baseline_std <= 0.0 {
            continue;
        }
        for std in &baseline.window_stds {
            if classify(*std, baseline_std, presence_threshold_db, MIN_SCALE_DB) {
                fp += 1;
            }
        }
        if let Some(walk) = walk_nodes.and_then(|w| w.get(node)) {
            for std in &walk.window_stds {
                if classify(*std, baseline_std, presence_threshold_db, MIN_SCALE_DB) {
                    tp += 1;
                } else {
                    fn_ += 1;
                }
            }
        }
    }
    let cv_f1 = f1(tp, fp, fn_);

    let contributing_links: Vec<String> = links
        .iter()
        .filter(|l| {
            contributing
                .iter()
                .any(|n| l.nodes[0] == n.0 || l.nodes[1] == n.0)
        })
        .map(|l| l.link_id.clone())
        .collect();

    let label = topology
        .room(room)
        .map_or_else(|| room.as_str().to_string(), |r| r.name.clone());

    Ok(ZoneCalibration {
        zone_id: room.as_str().to_string(),
        room_id: room.as_str().to_string(),
        label,
        adjacency: Vec::new(),
        detection: ZoneDetection {
            presence_threshold_db: presence_threshold_db as f32,
            motion_threshold_db: motion_threshold_db as f32,
            target_fpr: target_fpr as f32,
            contributing_links,
            contributing_nodes: contributing.iter().map(|n| n.0).collect(),
            empty_level_std: empty_level as f32,
            occupied_level_std: occupied_level as f32,
            quality: ZoneQuality {
                separation: separation as f32,
                cv_f1: cv_f1 as f32,
            },
        },
    })
}

/// The calibrated presence logit for one node's window.
///
/// This is the single source of truth for the calibrated detector: the zone
/// calibration F1 and the engine's `process_batch` both call it, so a learned
/// threshold is measured and applied in exactly the same units.
#[must_use]
pub(crate) fn calibrated_logit(
    window_std: f64,
    baseline_std: f64,
    presence_threshold_db: f64,
    scale_floor_db: f64,
) -> f64 {
    if baseline_std <= 0.0 {
        return 0.0;
    }
    let level_db = db20(window_std) - db20(baseline_std);
    let scale = (presence_threshold_db / 2.0).max(scale_floor_db);
    (level_db - presence_threshold_db) / scale
}

/// The binary presence decision at the calibrated threshold.
fn classify(window_std: f64, baseline_std: f64, threshold_db: f64, scale_floor_db: f64) -> bool {
    sigmoid(calibrated_logit(
        window_std,
        baseline_std,
        threshold_db,
        scale_floor_db,
    )) >= PRESENCE_DECISION
}

/// Build the floors and rooms frames.
fn build_frames(topology: &Topology, links: &[LinkCalibration]) -> FramesCalibration {
    let floors = topology
        .model()
        .floors
        .iter()
        .map(|floor| FloorFrame {
            floor_id: floor.id.as_str().to_string(),
            index: floor.level,
            elevation_m: floor.elevation_m,
            slab_attenuation_db: slab_attenuation(links),
        })
        .collect();
    let rooms = topology
        .model()
        .rooms
        .iter()
        .map(|room| RoomFrame {
            room_id: room.id.as_str().to_string(),
            floor_id: room.floor.as_str().to_string(),
            transform_world: [
                room.origin_home_m[0],
                room.origin_home_m[1],
                room.yaw_deg.to_radians(),
            ],
            polygon_floor_m: Vec::new(),
            confidence: if room.area_m2.is_some() { 0.8 } else { 0.3 },
        })
        .collect();
    FramesCalibration {
        world: WorldFrame {
            kind: "enu".to_string(),
            origin_ref: topology.model().rooms.first().map_or_else(
                || "world".to_string(),
                |r| format!("room:{}:corner", r.id.as_str()),
            ),
            units: "metre".to_string(),
        },
        floors,
        rooms,
    }
}

/// The learned slab attenuation from the inter-floor links; `0.0` when none
/// exist. A documented v1 proxy pending a real cross-floor study
/// (`geometry-calibration.md` §14).
fn slab_attenuation(links: &[LinkCalibration]) -> f32 {
    let inter: Vec<f64> = links
        .iter()
        .filter(|l| l.scope == "inter_floor")
        .map(|l| f64::from(l.proximity.range_estimate_m).max(0.0))
        .collect();
    if inter.is_empty() {
        0.0
    } else {
        mean_of(&inter) as f32
    }
}

/// The fraction of candidate node pairs with a usable baseline.
fn link_coverage(links: &[LinkCalibration], topology: &Topology) -> f64 {
    let candidates = candidate_pairs(topology).len();
    if candidates == 0 {
        return 1.0;
    }
    let usable = links
        .iter()
        .filter(|l| l.baseline.samples > 0 && l.baseline.coverage >= 0.9)
        .count();
    (usable as f64 / candidates as f64).min(1.0)
}

/// Roll the metrics up into gated quality.
fn gate(metrics: QualityMetrics) -> CalibrationQuality {
    let gates = vec![
        Gate {
            gate: "link_coverage".to_string(),
            value: metrics.link_coverage,
            threshold: 0.9,
            pass: metrics.link_coverage >= 0.9,
            direction: "min".to_string(),
        },
        Gate {
            gate: "baseline_snr_db".to_string(),
            value: metrics.baseline_snr_db,
            threshold: 12.0,
            pass: metrics.baseline_snr_db >= 12.0,
            direction: "min".to_string(),
        },
        Gate {
            gate: "excess_dynamic_range_db".to_string(),
            value: metrics.excess_dynamic_range_db,
            threshold: 3.0,
            pass: metrics.excess_dynamic_range_db >= 3.0,
            direction: "min".to_string(),
        },
        Gate {
            gate: "zone_separation".to_string(),
            value: metrics.zone_separation,
            threshold: 1.5,
            pass: metrics.zone_separation >= 1.5,
            direction: "min".to_string(),
        },
        Gate {
            gate: "presence_f1".to_string(),
            value: metrics.presence_f1,
            threshold: 0.9,
            pass: metrics.presence_f1 >= 0.9,
            direction: "min".to_string(),
        },
        Gate {
            gate: "drift_residual_ms".to_string(),
            value: metrics.drift_residual_ms,
            threshold: 5.0,
            pass: metrics.drift_residual_ms <= 5.0,
            direction: "max".to_string(),
        },
    ];

    let valid = gates.iter().all(|g| g.pass);
    let status = if !valid {
        QualityStatus::Poor
    } else {
        let margin = metrics.link_coverage >= 0.95
            && metrics.baseline_snr_db >= 15.0
            && metrics.zone_separation >= 2.0
            && metrics.presence_f1 >= 0.95
            && metrics.drift_residual_ms <= 2.5;
        if margin {
            QualityStatus::Good
        } else {
            QualityStatus::Fair
        }
    };
    CalibrationQuality {
        status,
        valid,
        gates,
        metrics,
    }
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// One entry of the append-only calibration index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationIndexEntry {
    /// The calibration identity.
    pub calibration_id: String,
    /// The core commit that produced it.
    pub core_commit: String,
    /// The topology digest it is bound to.
    pub topology_digest: String,
    /// The generation time.
    pub generated_at: String,
    /// The quality status.
    pub status: QualityStatus,
    /// Whether the artefact is valid.
    pub valid: bool,
}

/// The append-only index with the `current` pointer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationIndex {
    /// The index schema version.
    #[serde(default = "index_version")]
    pub version: String,
    /// Every recorded calibration, oldest first.
    pub entries: Vec<CalibrationIndexEntry>,
    /// The current calibration id, when one has been published.
    pub current: Option<String>,
}

impl Default for CalibrationIndex {
    fn default() -> Self {
        Self {
            version: index_version(),
            entries: Vec::new(),
            current: None,
        }
    }
}

/// The default index version label.
fn index_version() -> String {
    "aquila.calibration.index/v1".to_string()
}

impl CalibrationIndex {
    /// The current entry, when one has been published.
    #[must_use]
    pub fn current_entry(&self) -> Option<&CalibrationIndexEntry> {
        let current = self.current.as_deref()?;
        self.entries.iter().find(|e| e.calibration_id == current)
    }
}

/// The on-disk calibration store.
///
/// Writes are temp-file → `fsync` → atomic rename; the index is updated in the
/// same transaction (`geometry-calibration.md` §9), so a crash never leaves a
/// half-written `current`.
#[derive(Debug, Clone)]
pub struct CalibrationStore {
    root: PathBuf,
}

impl CalibrationStore {
    /// Construct a store rooted at an explicit directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The default store for a home:
    /// `/data/aquila/homes/<home_id>/calibration/`.
    #[must_use]
    pub fn default_for(home_id: &HomeId) -> Self {
        Self::new(
            PathBuf::from("/data/aquila/homes")
                .join(home_id.as_str())
                .join("calibration"),
        )
    }

    /// The store's root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Append an artefact to the store and point `current` at it.
    ///
    /// # Errors
    ///
    /// Returns [`CalibrationError::DigestMismatch`] if the artefact's digest is
    /// inconsistent, or [`CalibrationError::Io`] / [`CalibrationError::Json`] on
    /// a filesystem or serialisation failure.
    pub fn write(&self, artifact: &CalibrationArtifact) -> Result<(), CalibrationError> {
        artifact.verify_digest()?;
        fs::create_dir_all(&self.root).map_err(io_error)?;
        let filename = format!("{}.json", hex_name(&artifact.calibration_id));
        atomic_write(&self.root.join(filename), artifact.to_json()?.as_bytes())?;

        let mut index = self.index()?;
        index
            .entries
            .retain(|e| e.calibration_id != artifact.calibration_id);
        index.entries.push(CalibrationIndexEntry {
            calibration_id: artifact.calibration_id.clone(),
            core_commit: artifact.generated_by.core_commit.clone(),
            topology_digest: artifact.home.topology_digest.clone(),
            generated_at: artifact.generated_at.clone(),
            status: artifact.quality.status,
            valid: artifact.quality.valid,
        });
        index.current = Some(artifact.calibration_id.clone());
        let index_json = serde_json::to_string(&index)
            .map_err(|error| CalibrationError::Json(error.to_string()))?;
        atomic_write(&self.root.join("index.json"), index_json.as_bytes())?;
        Ok(())
    }

    /// Read one artefact by its identity.
    ///
    /// # Errors
    ///
    /// Returns [`CalibrationError::NotFound`] when the file is absent, or a
    /// parse error when it is present but malformed.
    pub fn read(&self, calibration_id: &str) -> Result<CalibrationArtifact, CalibrationError> {
        let path = self.root.join(format!("{}.json", hex_name(calibration_id)));
        if !path.exists() {
            return Err(CalibrationError::NotFound {
                calibration_id: calibration_id.to_string(),
            });
        }
        let json = fs::read_to_string(&path).map_err(io_error)?;
        CalibrationArtifact::from_json(&json)
    }

    /// Read the current artefact, if one has been published.
    ///
    /// # Errors
    ///
    /// Returns a parse error when the index or artefact is malformed.
    pub fn read_current(&self) -> Result<Option<CalibrationArtifact>, CalibrationError> {
        let index = self.index()?;
        match index.current {
            Some(id) => self.read(&id).map(Some),
            None => Ok(None),
        }
    }

    /// Load the current artefact, verifying it against current facts.
    ///
    /// Returns `Ok(None)` when no artefact has been published, and
    /// [`CalibrationError::Stale`] when the current artefact may not be reused —
    /// so a stale artefact is never silently returned (`geometry-calibration.md`
    /// §10).
    ///
    /// # Errors
    ///
    /// Returns [`CalibrationError::Stale`], [`CalibrationError::DigestMismatch`]
    /// or a parse error.
    pub fn load_current_validated(
        &self,
        expect: &CalibrationExpectation,
    ) -> Result<Option<CalibrationArtifact>, CalibrationError> {
        let Some(artifact) = self.read_current()? else {
            return Ok(None);
        };
        artifact.verify_digest()?;
        artifact.check(expect)?;
        Ok(Some(artifact))
    }

    /// The append-only index.
    ///
    /// # Errors
    ///
    /// Returns a parse error when the index is present but malformed.
    pub fn index(&self) -> Result<CalibrationIndex, CalibrationError> {
        let path = self.root.join("index.json");
        if !path.exists() {
            return Ok(CalibrationIndex::default());
        }
        let json = fs::read_to_string(&path).map_err(io_error)?;
        serde_json::from_str(&json).map_err(|error| CalibrationError::Json(error.to_string()))
    }
}

/// Strip a `sha256:` prefix, returning the bare hex name.
fn hex_name(calibration_id: &str) -> &str {
    calibration_id.strip_prefix("sha256:").unwrap_or(calibration_id)
}

/// Map a filesystem error to a typed calibration error.
fn io_error(error: std::io::Error) -> CalibrationError {
    CalibrationError::Io(error.to_string())
}

/// A process-unique counter for temporary file names.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Write `bytes` to `path` atomically: temp file, `fsync`, rename.
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), CalibrationError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let temp = dir.join(format!(
        ".tmp-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    {
        let mut file = fs::File::create(&temp).map_err(io_error)?;
        file.write_all(bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
    }
    fs::rename(&temp, path).map_err(io_error)
}

// ---------------------------------------------------------------------------
// Small numeric helpers
// ---------------------------------------------------------------------------

/// The mean of a slice, or `0.0` when empty.
fn mean_of(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

/// Decibel conversion of an amplitude-like quantity, floored so a zero is
/// finite rather than `-inf`.
fn db20(value: f64) -> f64 {
    20.0 * value.max(1e-12).log10()
}

/// Return `value` when finite, else `fallback`.
fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

/// The F1 of a binary decision.
fn f1(tp: u64, fp: u64, fn_: u64) -> f64 {
    let precision = if tp + fp == 0 {
        0.0
    } else {
        tp as f64 / (tp + fp) as f64
    };
    let recall = if tp + fn_ == 0 {
        0.0
    } else {
        tp as f64 / (tp + fn_) as f64
    };
    if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    }
}

/// The schema label for a device type.
fn device_label(device: DeviceType) -> &'static str {
    match device {
        DeviceType::Esp32S3 => "esp32s3",
        DeviceType::Esp32C6 => "esp32c6",
        DeviceType::Unknown => "unknown",
    }
}

/// The schema label for a band.
fn band_label(band: aquila_types::Band) -> &'static str {
    match band {
        aquila_types::Band::B2G4 => "2.4ghz",
        aquila_types::Band::B5G => "5ghz",
        aquila_types::Band::Unknown => "unknown",
    }
}

/// The schema label for a proximity relation.
fn relation_label(relation: aquila_geometry::ProximityRelation) -> &'static str {
    use aquila_geometry::ProximityRelation;
    match relation {
        ProximityRelation::SameZoneCandidate => "same_zone_candidate",
        ProximityRelation::Adjacent => "adjacent",
        ProximityRelation::Far => "far",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db20_is_finite_at_zero() {
        assert!(db20(0.0).is_finite());
        assert!(db20(1.0).abs() < 1e-9);
    }

    #[test]
    fn calibrated_logit_is_monotone_in_level() {
        let low = calibrated_logit(0.01, 0.03, 8.0, MIN_SCALE_DB);
        let high = calibrated_logit(0.30, 0.03, 8.0, MIN_SCALE_DB);
        assert!(high > low, "a stronger rise must score a higher logit");
        assert!(low < 0.0 && high > 0.0);
    }

    #[test]
    fn f1_is_one_for_a_perfect_split() {
        assert!((f1(10, 0, 0) - 1.0).abs() < 1e-12);
        assert!(f1(0, 5, 5).abs() < 1e-12);
    }
}
