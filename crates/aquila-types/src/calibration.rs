//! Time-sync, calibration and baseline contracts.
//!
//! [`ClockModel`] is the per-node software time-sync estimate the `Clock` seam
//! produces. [`Calibration`] is the global, whole-home process that corrects
//! node and node-pair CSI to a common reference, and the stored corrections it
//! produces. [`Baseline`] is the reference state recorded with no person
//! present, against which deviations are measured.

use serde::{Deserialize, Serialize};

use crate::error::{invariant, require_finite_f32, require_finite_f64, ValidationError};
use crate::ids::{HomeId, NodeId, PairId, RoomId, SessionId};
use crate::primitives::Witness;
use crate::time::TimeSpan;
use crate::version::ContractVersion;

/// A per-node software time-sync estimate produced by the `Clock` seam.
///
/// The affine map is
/// `host_mono_ns = device_time_us * 1000 + offset_ns + drift_ppm * elapsed`.
/// The published `uncertainty_ns` is a bound the core honours.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClockModel {
    /// The node the model corrects.
    pub node: NodeId,
    /// The boot epoch the model applies to.
    pub boot_epoch: u16,
    /// The offset at the epoch, in nanoseconds.
    pub offset_ns: i64,
    /// The relative rate error, in parts per million.
    pub drift_ppm: f64,
    /// The uncertainty bound, in nanoseconds.
    pub uncertainty_ns: u64,
    /// The monotonic time the model was estimated at, in nanoseconds.
    pub estimated_mono_ns: u64,
    /// The number of accepted samples behind the fit.
    pub samples: u64,
    /// The evidence grade.
    pub witness: Witness,
}

impl ClockModel {
    /// Validate the node identity and the finiteness of the drift estimate.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.node.validate_provisioned()?;
        require_finite_f64("ClockModel.drift_ppm", self.drift_ppm)
    }
}

/// A per-node-pair correction: an amplitude and phase offset.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PairCorrection {
    /// The amplitude correction, in decibels.
    pub amplitude_db: f32,
    /// The phase correction, in radians.
    pub phase_rad: f32,
}

impl PairCorrection {
    /// Validate the finiteness of both corrections.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_finite_f32("PairCorrection.amplitude_db", self.amplitude_db)?;
        require_finite_f32("PairCorrection.phase_rad", self.phase_rad)
    }
}

/// Global, whole-home calibration: the stored corrections produced by the
/// per-home calibration process.
///
/// v1 is global — one calibration for the whole home — not per-room. A
/// calibration is bound to `topology_hash`; a topology change invalidates it
/// and emits `CalibrationChanged`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    /// The home the calibration belongs to.
    pub home: HomeId,
    /// The contract version.
    pub contract: ContractVersion,
    /// Unix wall time the calibration was created, in milliseconds.
    pub created_unix_ms: i64,
    /// The session the calibration was derived from.
    pub session: SessionId,
    /// SHA-256 of the topology the calibration corrects.
    pub topology_hash: [u8; 32],
    /// Per-node positions in the home frame, in metres.
    pub node_positions_m: Vec<(NodeId, [f32; 3])>,
    /// Per-node gain corrections, in decibels.
    pub node_offsets_db: Vec<(NodeId, f32)>,
    /// Per-pair corrections.
    pub pair_offsets: Vec<(PairId, PairCorrection)>,
    /// The span over which the calibration is valid.
    pub validity: TimeSpan,
    /// The evidence grade.
    pub witness: Witness,
}

impl Calibration {
    /// Validate identity, finiteness and the validity span.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.home.validate()?;
        self.session.validate()?;
        self.validity.validate()?;
        for (node, pos) in &self.node_positions_m {
            node.validate_provisioned()?;
            for c in pos {
                require_finite_f32("Calibration.node_positions_m", *c)?;
            }
        }
        for (node, offset) in &self.node_offsets_db {
            node.validate_provisioned()?;
            require_finite_f32("Calibration.node_offsets_db", *offset)?;
        }
        for (pair, correction) in &self.pair_offsets {
            pair.validate()?;
            correction.validate()?;
        }
        Ok(())
    }
}

/// The scope of a baseline: a zone, or a node-pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineScope {
    /// A zone's baseline, keyed by the room that is the zone in v1.
    Zone(RoomId),
    /// A node-pair's baseline.
    Pair(PairId),
}

impl BaselineScope {
    /// Validate that the carried identifier is non-empty.
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            BaselineScope::Zone(id) => id.validate(),
            BaselineScope::Pair(id) => id.validate(),
        }
    }
}

/// A reference state recorded with no person present, against which deviations
/// are measured. One per scope.
///
/// A baseline is invalidated by a topology or calibration change that alters
/// its scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    /// The home the baseline belongs to.
    pub home: HomeId,
    /// The session the baseline was recorded in.
    pub session: SessionId,
    /// The scope the baseline covers.
    pub scope: BaselineScope,
    /// The span the baseline covers.
    pub span: TimeSpan,
    /// Per-subcarrier reference amplitude, in decibels.
    pub amplitude_ref_db: Vec<f32>,
    /// Per-subcarrier reference phase, in radians (uncalibrated, same node).
    pub phase_ref_rad: Vec<f32>,
    /// Unix wall time the baseline was created, in milliseconds.
    pub created_unix_ms: i64,
    /// The evidence grade.
    pub witness: Witness,
}

impl Baseline {
    /// Validate identity, the scope, the span and the per-subcarrier references.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.home.validate()?;
        self.session.validate()?;
        self.scope.validate()?;
        self.span.validate()?;
        if self.amplitude_ref_db.len() != self.phase_ref_rad.len() {
            return Err(invariant(
                "Baseline amplitude and phase references must have equal length",
            ));
        }
        for a in &self.amplitude_ref_db {
            require_finite_f32("Baseline.amplitude_ref_db", *a)?;
        }
        for p in &self.phase_ref_rad {
            require_finite_f32("Baseline.phase_ref_rad", *p)?;
        }
        Ok(())
    }
}
