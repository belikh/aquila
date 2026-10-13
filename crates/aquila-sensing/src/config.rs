//! The sensing core's configuration.
//!
//! Every threshold, window length and clamp lives here and none is hidden as a
//! literal (`fusion-sync.md` Appendix A). The defaults are the slice-1 values:
//! window assembly on a one-second hop, room-scope presence and count, and a
//! single-window hysteresis. Later slices thicken the escalation policy without
//! moving a constant out of this struct.
//!
//! Slice 3 adds the **calibrated-mode inputs**: an optional
//! [`CalibrationArtifact`] whose per-node clock corrections and per-zone
//! thresholds override the uncalibrated defaults when present and valid, plus a
//! floor on the calibrated logit scale.

use crate::calibration::CalibrationArtifact;
use crate::fusion::{TorsoGeometry, TORSO_KEYPOINT_SCATTERS};
use crate::pose::DEFAULT_TORSO_DITHER_HZ;

/// Provenance recorded on every [`Measurement`](aquila_metrics::Measurement) the
/// core writes.
///
/// A measurement is commit-keyed, so the commit and host travel with the record
/// rather than being read from the environment by the core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementConfig {
    /// The full 40-character hex commit the measurement is keyed to.
    pub commit: String,
    /// A stable host fingerprint.
    pub host: String,
    /// The metric-name prefix, for example `"sensing"`.
    pub prefix: String,
}

impl Default for MeasurementConfig {
    fn default() -> Self {
        Self {
            commit: "0".repeat(40),
            host: "aquila-sensing".to_string(),
            prefix: "sensing".to_string(),
        }
    }
}

/// The complete configuration of the sensing core.
///
/// All fields are public so a test or the composition root can pin them; the
/// values are validated once by [`SensingConfig::validate`].
#[derive(Debug, Clone, PartialEq)]
pub struct SensingConfig {
    /// The window hop: the core-time length of one analysis window, in
    /// nanoseconds. A frame belongs to the window its arrival falls in.
    pub window_hop_ns: u64,
    /// The spacing of the per-window clock grid, in nanoseconds.
    pub grid_interval_ns: u64,
    /// The number of slots on the per-window clock grid.
    pub grid_slots: u32,
    /// The minimum contributing frames a node needs before it can vote.
    pub min_frames_per_node: usize,
    /// The per-node presence-motion threshold: the temporal standard deviation
    /// of the window's amplitude (averaged over subcarriers) at which a node's
    /// evidence is odds 1:1. A person's motion and breathing make the CSI
    /// amplitude fluctuate above the static multipath, so that variability is
    /// the presence cue; a raw mean-amplitude threshold is not scale-robust.
    pub presence_motion_threshold: f64,
    /// The log-odds scale of the presence motion: one unit of amplitude
    /// standard deviation above the threshold adds `1 / presence_motion_scale`
    /// logits.
    pub presence_motion_scale: f64,
    /// The pooled prior log-odds for an uncalibrated room; `0` means `p = 0.5`
    /// ("unknown"), never "absent" (`fusion-sync.md` §2.2).
    pub prior_logit: f64,
    /// The per-node log-odds clamp (`L_max`), so no single node can force
    /// occupancy.
    pub logit_clamp: f64,
    /// The pooled presence probability below which an occupied room becomes
    /// absent.
    pub p_lo: f64,
    /// The pooled presence probability above which an absent room becomes
    /// occupied.
    pub p_hi: f64,
    /// The number of consecutive windows a state change must hold before it is
    /// committed (`T_hold`).
    pub t_hold: u32,
    /// The room-attribution ambiguity margin (`fusion-sync.md` §2.4.1): when a
    /// floor is occupied and the two strongest room presence probabilities
    /// differ by less than this, the floor's room attribution is withheld and a
    /// [`Conflict::AmbiguousRooms`](crate::Conflict) is raised rather than
    /// naming a false confident room.
    pub room_gap: f64,
    /// The v1 cap on a people count.
    pub max_count: u8,
    /// The base fusion weight of a healthy node.
    pub node_weight: f64,
    /// Whether the core estimates vitals (breathing and heart rate) per window
    /// (slice 2). Off by default, so slice 1 emits presence and count only.
    pub vitals_enabled: bool,
    /// The accumulation span of one vitals estimate, in nanoseconds. Vital-rate
    /// estimation needs several respiratory cycles, so this is longer than the
    /// presence window hop. Vitals are computed from the same frames as
    /// presence, accumulated over this span.
    pub vitals_window_ns: u64,
    /// The validated whole-home calibration artefact to apply, when present.
    ///
    /// When `Some` and valid, it supplies per-node clock correction (seeded into
    /// the core's time sync) and per-zone presence thresholds, expressed as a
    /// decibel rise above each node's empty-home baseline. When `None`, the
    /// uncalibrated `presence_motion_threshold` / `presence_motion_scale`
    /// defaults apply.
    pub calibration: Option<CalibrationArtifact>,
    /// The floor on the calibrated logit scale, in dB per logit, so a tiny
    /// learned zone threshold cannot amplify noise without bound.
    pub calibration_scale_floor_db: f64,
    /// Whether the core estimates coarse pose per room (Slice 4). Off by
    /// default, so a configuration that does not opt in emits no pose.
    pub pose_enabled: bool,
    /// The minimum contributing frames per node before a pose is attempted.
    pub pose_min_frames: usize,
    /// The per-scatterer pose-dither frequencies, in hertz, in torso scatterer
    /// order (left shoulder, right shoulder, left hip, right hip). They must
    /// match the synthetic signature the observed CSI carries.
    pub pose_dither_hz: [f64; TORSO_KEYPOINT_SCATTERS],
    /// The rigid torso geometry the coarse-pose combination fits.
    pub pose_geometry: TorsoGeometry,
    /// The calibration quality `q_n` attributed to a node that carries no
    /// per-node calibration quality, that is, when no calibration artefact is
    /// applied. The default `0.0` enforces `fusion-sync.md` §2.3's restraint
    /// (an uncalibrated node contributes no pose); a synthetic in-domain
    /// fixture that treats its nodes as calibrated opts in explicitly.
    pub pose_uncalibrated_quality: f64,
    /// The minimum confidence a room's vital estimate must carry to be eligible
    /// for the floor's highest-confidence selection (`fusion-sync.md` §2.2).
    /// The default `0.0` admits every finite estimate.
    pub vitals_floor_min_confidence: f64,
    /// The provenance recorded on each measurement.
    pub measurement: MeasurementConfig,
}

impl Default for SensingConfig {
    fn default() -> Self {
        Self {
            window_hop_ns: 1_000_000_000,
            grid_interval_ns: 50_000_000,
            grid_slots: 20,
            min_frames_per_node: 3,
            // The motion threshold is tuned to the deterministic simulator's
            // amplitude scale; it is a starting point, not a claim. The cue is
            // the window's amplitude variability: a moving occupant measures
            // ~0.22 and an empty room ~0.03 (the physical simulator of
            // `capture-and-firmware.md` §7.2), so 0.10 sits between them.
            // Global calibration (slice 3) removes the need for an absolute
            // per-home level.
            presence_motion_threshold: 0.10,
            presence_motion_scale: 0.03,
            prior_logit: 0.0,
            logit_clamp: 4.0,
            p_lo: 0.2,
            p_hi: 0.8,
            t_hold: 1,
            room_gap: 0.15,
            max_count: aquila_types::MAX_COUNT_V1,
            node_weight: 1.0,
            vitals_enabled: false,
            // Fifteen seconds is ~3.7 respiratory cycles at 0.25 Hz: enough to
            // resolve the rate, and short enough to track it.
            vitals_window_ns: 15_000_000_000,
            calibration: None,
            calibration_scale_floor_db: 1.0,
            pose_enabled: false,
            pose_min_frames: 8,
            pose_dither_hz: DEFAULT_TORSO_DITHER_HZ,
            pose_geometry: TorsoGeometry::adult(),
            pose_uncalibrated_quality: 0.0,
            vitals_floor_min_confidence: 0.0,
            measurement: MeasurementConfig::default(),
        }
    }
}

impl SensingConfig {
    /// Validate the configuration's internal consistency.
    ///
    /// # Errors
    ///
    /// Returns [`SensingError::Config`](crate::SensingError::Config) naming the
    /// first broken invariant: a zero hop or grid interval, a zero slot count, a
    /// non-finite or mis-ordered threshold, or a non-positive scale or clamp.
    pub fn validate(&self) -> Result<(), crate::SensingError> {
        let bad = |detail: &str| crate::SensingError::Config(detail.to_string());
        if self.window_hop_ns == 0 {
            return Err(bad("window_hop_ns must be non-zero"));
        }
        if self.grid_interval_ns == 0 || self.grid_slots == 0 {
            return Err(bad(
                "the clock grid must have a positive interval and slots",
            ));
        }
        if self.min_frames_per_node == 0 {
            return Err(bad("min_frames_per_node must be at least 1"));
        }
        for (name, value) in [
            ("presence_motion_threshold", self.presence_motion_threshold),
            ("presence_motion_scale", self.presence_motion_scale),
            ("prior_logit", self.prior_logit),
            ("logit_clamp", self.logit_clamp),
            ("p_lo", self.p_lo),
            ("p_hi", self.p_hi),
            ("node_weight", self.node_weight),
            ("calibration_scale_floor_db", self.calibration_scale_floor_db),
        ] {
            if !value.is_finite() {
                return Err(bad(&format!("{name} must be finite")));
            }
        }
        if self.calibration_scale_floor_db <= 0.0 {
            return Err(bad("calibration_scale_floor_db must be positive"));
        }
        if self.presence_motion_scale <= 0.0 {
            return Err(bad("presence_motion_scale must be positive"));
        }
        if self.logit_clamp <= 0.0 {
            return Err(bad("logit_clamp must be positive"));
        }
        if self.node_weight <= 0.0 {
            return Err(bad("node_weight must be positive"));
        }
        if !(0.0..=1.0).contains(&self.p_lo) || !(0.0..=1.0).contains(&self.p_hi) {
            return Err(bad("p_lo and p_hi must lie in [0, 1]"));
        }
        if self.p_lo >= self.p_hi {
            return Err(bad("p_lo must be below p_hi"));
        }
        if !(0.0..=1.0).contains(&self.room_gap) {
            return Err(bad("room_gap must lie in [0, 1]"));
        }
        if !(0.0..=1.0).contains(&self.vitals_floor_min_confidence) {
            return Err(bad("vitals_floor_min_confidence must lie in [0, 1]"));
        }
        if self.max_count == 0 || self.max_count > aquila_types::MAX_COUNT_V1 {
            return Err(bad("max_count must lie in 1..=MAX_COUNT_V1"));
        }
        if self.vitals_enabled && self.vitals_window_ns == 0 {
            return Err(bad(
                "vitals_window_ns must be non-zero when vitals are enabled",
            ));
        }
        if self.pose_enabled {
            if self.pose_min_frames < 4 {
                return Err(bad("pose_min_frames must be at least 4"));
            }
            for (j, frequency) in self.pose_dither_hz.iter().enumerate() {
                if !frequency.is_finite() || *frequency <= 0.0 {
                    return Err(bad(&format!(
                        "pose_dither_hz[{j}] must be finite and positive"
                    )));
                }
            }
            let geometry = &self.pose_geometry;
            for (name, value) in [
                ("pose_geometry.shoulder_half_width_m", geometry.shoulder_half_width_m),
                ("pose_geometry.hip_half_width_m", geometry.hip_half_width_m),
                ("pose_geometry.torso_length_m", geometry.torso_length_m),
            ] {
                if !value.is_finite() || value <= 0.0 {
                    return Err(bad(&format!("{name} must be finite and positive")));
                }
            }
            if !self.pose_uncalibrated_quality.is_finite()
                || !(0.0..=1.0).contains(&self.pose_uncalibrated_quality)
            {
                return Err(bad("pose_uncalibrated_quality must lie in [0, 1]"));
            }
        }
        if self.measurement.commit.len() != 40
            || !self
                .measurement
                .commit
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(bad(
                "measurement.commit must be a full 40-character hex SHA",
            ));
        }
        if self.measurement.host.is_empty() {
            return Err(bad("measurement.host must be non-empty"));
        }
        if self.measurement.prefix.is_empty() {
            return Err(bad("measurement.prefix must be non-empty"));
        }
        Ok(())
    }
}
