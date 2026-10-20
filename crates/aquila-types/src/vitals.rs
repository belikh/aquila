//! Vitals: windowed breathing-rate and heart-rate estimates.
//!
//! Breathing rate and heart rate are *separate* estimates with their own
//! confidence; either may be absent. No HRV, SpO2 or rhythm analysis is in
//! scope. A reading whose `status` is not `Ok` must not report a value.

use serde::{Deserialize, Serialize};

use crate::error::{require_finite_f32, require_unit_interval, ValidationError};
use crate::ids::{SubjectId, ZoneId};
use crate::primitives::Witness;
use crate::time::TimeSpan;

/// One physiological estimate: a value and the model's own certainty.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Estimate {
    /// The value, in the units of the field that carries it (brpm or bpm).
    pub value: f32,
    /// The model's confidence in `[0, 1]`.
    pub confidence: f32,
}

impl Estimate {
    /// Validate finiteness and the confidence range.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_finite_f32("Estimate.value", self.value)?;
        require_unit_interval("Estimate.confidence", self.confidence)
    }
}

/// The status of a vitals reading. `Unknown` is a first-class fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VitalsStatus {
    /// The reading is usable.
    Ok,
    /// The SNR is below the estimability floor.
    InsufficientSignal,
    /// Movement corrupts the phase history.
    MotionArtifact,
    /// Nobody is present in scope.
    NoSubject,
    /// The calibration is older than the model permits.
    CalibrationStale,
    /// An unrecognised or unset status.
    #[serde(other)]
    Unknown = 255,
}

/// A windowed physiological estimate for one subject, or one zone when no
/// subject is available.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VitalsReading {
    /// The subject, or `None` for a zone-scoped reading.
    pub subject: Option<SubjectId>,
    /// The zone the reading is attributed to.
    pub zone: ZoneId,
    /// The window the estimate covers.
    pub span: TimeSpan,
    /// Breathing rate, in breaths per minute (brpm).
    pub breathing: Option<Estimate>,
    /// Heart rate, in beats per minute (bpm).
    pub heart_rate: Option<Estimate>,
    /// The status of the reading.
    pub status: VitalsStatus,
    /// The evidence grade.
    pub witness: Witness,
}

impl VitalsReading {
    /// Validate the status/estimate coupling and the plausible ranges.
    ///
    /// * `Ok` requires at least one estimate.
    /// * Any other status requires both estimates to be absent, so a reading
    ///   never simultaneously claims "no signal" and reports a value.
    /// * Breathing is plausible in `[4, 60]` brpm and heart rate in `[30, 220]`
    ///   bpm; the producer drops out-of-range values rather than emitting them.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.zone.validate()?;
        self.span.validate()?;

        if let Some(breathing) = &self.breathing {
            breathing.validate()?;
            if !(4.0..=60.0).contains(&breathing.value) {
                return Err(ValidationError::Range {
                    field: "VitalsReading.breathing.value",
                    detail: format!("{} is not in [4, 60] brpm", breathing.value),
                });
            }
        }
        if let Some(heart_rate) = &self.heart_rate {
            heart_rate.validate()?;
            if !(30.0..=220.0).contains(&heart_rate.value) {
                return Err(ValidationError::Range {
                    field: "VitalsReading.heart_rate.value",
                    detail: format!("{} is not in [30, 220] bpm", heart_rate.value),
                });
            }
        }

        match self.status {
            VitalsStatus::Ok => {
                if self.breathing.is_none() && self.heart_rate.is_none() {
                    return Err(ValidationError::Invariant {
                        detail: "VitalsStatus::Ok requires at least one estimate".to_string(),
                    });
                }
            }
            VitalsStatus::InsufficientSignal
            | VitalsStatus::MotionArtifact
            | VitalsStatus::NoSubject
            | VitalsStatus::CalibrationStale
            | VitalsStatus::Unknown => {
                if self.breathing.is_some() || self.heart_rate.is_some() {
                    return Err(ValidationError::Invariant {
                        detail: "a non-Ok VitalsStatus must not report a value".to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}
