//! Pose: a coarse, zone-scoped configuration of body keypoints.
//!
//! The keypoint schema is **COCO-17**, fixed, and never redefined. Pose is
//! low-fidelity by design. A missing keypoint is expressed as
//! `confidence == 0.0`; coordinates may be `NaN` to mean "no estimate", and
//! consumers must gate on `confidence` rather than trusting the coordinates.

use serde::{Deserialize, Serialize};

use crate::error::{require_not_infinite, require_unit_interval, ValidationError};
use crate::ids::{SubjectId, ZoneId};
use crate::primitives::Witness;

/// The number of keypoints in the COCO-17 schema.
pub const KEYPOINT_COUNT: usize = 17;

/// The number of coarse-pose torso scatterers: left and right shoulder, left
/// and right hip, in that order.
pub const TORSO_SCATTERER_COUNT: usize = 4;

/// The torso pose-dither band, in hertz, `[lo, hi]`.
///
/// The synthetic coarse-pose fixture gives each torso scatterer its own radial
/// micro-motion frequency, drawn **per seed** from this band. The band (and the
/// scatterer count) is the only pose-dither knowledge shared between the
/// simulator that injects the signature and the estimator that recovers it:
/// the estimator must *search* the band and find the actual peaks, so the pose
/// test is not keyed to a private per-scatterer constant. This lives in the
/// level-0 contract crate so the two sides share one declaration
/// (`sister-issues-core.md` S4, S7).
pub const TORSO_DITHER_BAND_HZ: [f64; 2] = [2.5, 9.5];

/// The minimum separation between two torso-scatterer dither frequencies, in
/// hertz, so a band search can resolve them as distinct peaks. The simulation
/// draws frequencies at least this far apart, and the estimator enforces the
/// same separation when it selects peaks.
pub const TORSO_DITHER_MIN_SEPARATION_HZ: f64 = 0.7;

/// A named body joint within a pose. The fixed COCO-17 order; discriminants are
/// the wire order and must never be renumbered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeypointId {
    /// Nose.
    Nose = 0,
    /// Left eye.
    LeftEye,
    /// Right eye.
    RightEye,
    /// Left ear.
    LeftEar,
    /// Right ear.
    RightEar,
    /// Left shoulder.
    LeftShoulder,
    /// Right shoulder.
    RightShoulder,
    /// Left elbow.
    LeftElbow,
    /// Right elbow.
    RightElbow,
    /// Left wrist.
    LeftWrist,
    /// Right wrist.
    RightWrist,
    /// Left hip.
    LeftHip,
    /// Right hip.
    RightHip,
    /// Left knee.
    LeftKnee,
    /// Right knee.
    RightKnee,
    /// Left ankle.
    LeftAnkle,
    /// Right ankle.
    RightAnkle,
}

/// One keypoint estimate, in room-local metres with a confidence.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Keypoint {
    /// Which joint this is.
    pub id: KeypointId,
    /// Room-local x, in metres. May be `NaN` to mean "no estimate".
    pub x_m: f32,
    /// Room-local y, in metres. May be `NaN` to mean "no estimate".
    pub y_m: f32,
    /// Room-local z, in metres; `None` when the model is planar.
    pub z_m: Option<f32>,
    /// Confidence in `[0, 1]`.
    pub confidence: f32,
}

impl Keypoint {
    /// Validate the confidence range and reject infinite coordinates.
    ///
    /// `NaN` coordinates are permitted and mean "no estimate"; infinities are
    /// not.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_unit_interval("Keypoint.confidence", self.confidence)?;
        require_not_infinite("Keypoint.x_m", self.x_m)?;
        require_not_infinite("Keypoint.y_m", self.y_m)?;
        if let Some(z) = self.z_m {
            require_not_infinite("Keypoint.z_m", z)?;
        }
        Ok(())
    }
}

/// A coarse, zone-scoped configuration of body keypoints for one subject at one
/// instant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    /// The session-scoped subject index.
    pub subject: SubjectId,
    /// The zone the pose is expressed in.
    pub zone: ZoneId,
    /// Unix wall time, in milliseconds.
    pub t_unix_ms: i64,
    /// Exactly [`KEYPOINT_COUNT`] keypoints in ascending [`KeypointId`] order.
    pub keys: [Keypoint; KEYPOINT_COUNT],
    /// Whole-subject confidence in `[0, 1]`.
    pub score: f32,
    /// The evidence grade.
    pub witness: Witness,
}

impl Pose {
    /// Validate the keypoint schema, ordering, confidence ranges and witness.
    ///
    /// The array *is* the schema: `keys[i].id as u8 == i` for every slot.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.zone.validate()?;
        require_unit_interval("Pose.score", self.score)?;
        for (i, key) in self.keys.iter().enumerate() {
            if key.id as usize != i {
                return Err(ValidationError::Invariant {
                    detail: "Pose.keys is not in ascending KeypointId order".to_string(),
                });
            }
            key.validate()?;
        }
        // Every witness grade is structurally valid on a pose; withholding a
        // low-`score` pose is a gate, not a witness value.
        Ok(())
    }
}
