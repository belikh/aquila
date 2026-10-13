//! Global whole-home calibration.
//!
//! Calibration is the per-home process that corrects node and node-pair CSI to a
//! common reference (`CONTEXT.md`, `geometry-calibration.md` §7). It is **out of
//! scope for slice 1**: the tracer bullet proves the end-to-end spine, not the
//! calibration wizard, which arrives in slice 3.
//!
//! [`Sensing::calibrate`](crate::Sensing::calibrate) is therefore a documented
//! stub: it reports honestly that no calibration was computed, records that fact
//! as a measurement, and never fabricates a correction. Uncalibrated nodes still
//! contribute presence; they contribute nothing to timing-sensitive branches,
//! which slice 1 does not yet run.

/// The outcome of a calibration attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationReport {
    /// Whether a calibration artefact was produced. Always `false` in slice 1.
    pub calibrated: bool,
    /// The number of windows observed so far, for provenance.
    pub windows_observed: usize,
    /// A human-readable explanation of the outcome.
    pub note: String,
}

impl CalibrationReport {
    /// The slice-1 stub outcome.
    #[must_use]
    pub fn stub(windows_observed: usize) -> Self {
        Self {
            calibrated: false,
            windows_observed,
            note: "calibration is a documented slice-1 stub; it arrives in slice 3".to_string(),
        }
    }
}
