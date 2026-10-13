//! Position estimation: proximity-first, with bounded trilateration as an
//! advisory secondary estimator.
//!
//! `geometry-calibration.md` §5 names two estimators in priority order:
//!
//! 1. **proximity fingerprinting** (primary, robust) — the discrete relation in
//!    [`crate::ProximityRelation`]; makes no metric distance claim;
//! 2. **RSSI trilateration** (secondary, advisory) — a weighted least-squares
//!    position with a 95 % error ellipse.
//!
//! The hard limit binds the second estimator: there is no cross-node phase
//! coherence and indoor RSSI is multipath-biased, so a path-loss "range" is
//! accurate to roughly a factor of two at best. Every continuous coordinate
//! produced here is therefore **advisory only**, carries an honest per-link
//! range sigma, and is demoted to room level when its 95 % ellipse exceeds the
//! room. It is never a hard input to presence, pose or vitals.

use aquila_types::{Confidence, NodeId, RoomId};
use serde::{Deserialize, Serialize};

use crate::confidence::unit_confidence;

/// Default reference RSSI at [`PathLossModel::reference_distance_m`], in dBm.
pub const DEFAULT_REFERENCE_RSSI_DBM: f32 = -40.0;
/// Default path-loss exponent, the midpoint of the v1 range `2.0..=3.5`.
pub const DEFAULT_PATH_LOSS_EXPONENT: f32 = 2.5;
/// Default reference distance for the path-loss model, in metres.
pub const DEFAULT_REFERENCE_DISTANCE_M: f32 = 1.0;
/// Minimum inversion range, in metres.
pub const MIN_RANGE_M: f32 = 0.1;
/// Maximum inversion range, in metres.
pub const MAX_RANGE_M: f32 = 100.0;
/// Per-link range sigma as a fraction of the range: the honest "factor of two"
/// bound from `geometry-calibration.md` §0.
pub const RANGE_SIGMA_FRACTION: f32 = 0.5;
/// Floor on the per-link range sigma, in metres.
pub const MIN_RANGE_SIGMA_M: f32 = 0.5;
/// Chi-square value for a 95 % confidence region with two degrees of freedom.
pub const CHI2_95_2DOF: f32 = 5.991_464_7;
/// Cap on advisory trilateration confidence: whatever the geometry, the
/// continuous estimate is advisory and never fully certain.
pub const TRILATERATION_CONFIDENCE_CAP: f32 = 0.6;
/// Fallback room extent, in metres, when a room has no surveyed area.
pub const DEFAULT_ROOM_EXTENT_M: f32 = 5.0;
/// Two attribution candidates closer than this are reported as `ambiguous`.
pub const AMBIGUITY_MARGIN: f32 = 0.1;
/// RSSI mapped to zero link confidence, in dBm.
pub const LINK_RSSI_FLOOR_DBM: f32 = -95.0;
/// RSSI span above [`LINK_RSSI_FLOOR_DBM`] mapped to full link confidence.
pub const LINK_RSSI_SPAN_DBM: f32 = 40.0;

/// A log-distance path-loss model, used only to turn an RSSI into an advisory
/// range.
///
/// `d = d_ref · 10 ^ ((rssi_ref − rssi) / (10 · n))`. Both `rssi_ref` and `n`
/// are defaulted placeholders in v1; `aquila-calibration` refines them from
/// links with known placements.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PathLossModel {
    /// Reference RSSI at the reference distance, in dBm.
    pub reference_rssi_dbm: f32,
    /// Path-loss exponent `n`.
    pub exponent: f32,
    /// Reference distance, in metres.
    pub reference_distance_m: f32,
}

impl Default for PathLossModel {
    fn default() -> Self {
        Self {
            reference_rssi_dbm: DEFAULT_REFERENCE_RSSI_DBM,
            exponent: DEFAULT_PATH_LOSS_EXPONENT,
            reference_distance_m: DEFAULT_REFERENCE_DISTANCE_M,
        }
    }
}

impl PathLossModel {
    /// Construct a model from its three parameters.
    #[must_use]
    pub const fn new(reference_rssi_dbm: f32, exponent: f32, reference_distance_m: f32) -> Self {
        Self {
            reference_rssi_dbm,
            exponent,
            reference_distance_m,
        }
    }

    /// Invert an RSSI to an advisory range, clamped to a sane interval.
    ///
    /// Returns `NaN` for a non-physical exponent; the caller treats that as "no
    /// range".
    #[must_use]
    pub fn range_m(&self, rssi_dbm: f32) -> f32 {
        if !self.exponent.is_finite()
            || self.exponent <= 0.0
            || !rssi_dbm.is_finite()
            || !self.reference_distance_m.is_finite()
        {
            return f32::NAN;
        }
        let ratio = (self.reference_rssi_dbm - rssi_dbm) / (10.0 * self.exponent);
        let distance = self.reference_distance_m * 10f32.powf(ratio);
        distance.clamp(MIN_RANGE_M, MAX_RANGE_M)
    }

    /// The forward path-loss model: the RSSI predicted at `range_m`.
    ///
    /// The inverse of [`PathLossModel::range_m`] over the unclamped interval.
    #[must_use]
    pub fn rssi_for_range_m(&self, range_m: f32) -> f32 {
        if !self.exponent.is_finite()
            || self.exponent <= 0.0
            || !range_m.is_finite()
            || range_m <= 0.0
        {
            return f32::NAN;
        }
        self.reference_rssi_dbm
            - 10.0 * self.exponent * (range_m / self.reference_distance_m).log10()
    }
}

/// A per-link confidence prior from RSSI, monotone in signal strength.
///
/// This is an uncalibrated placeholder: a stronger link indicates a stronger
/// local coupling, but the mapping is not learned and is capped well below
/// certainty. `aquila-calibration` replaces it with a per-zone signature
/// likelihood.
#[must_use]
pub fn link_confidence(rssi_dbm: f32) -> Confidence {
    unit_confidence((rssi_dbm - LINK_RSSI_FLOOR_DBM) / LINK_RSSI_SPAN_DBM)
}

/// Which estimator produced a [`PositionEstimate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionMethod {
    /// The primary, robust proximity fingerprint. No metric claim.
    ProximityFingerprint,
    /// The secondary, advisory RSSI trilateration.
    TrilaterationSecondary,
}

/// The honest resolution an estimate reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionResolution {
    /// Room-level: the guaranteed resolution of v1.
    Room,
    /// Zone-level: best-effort. In v1 a zone is a room, so this coincides with
    /// room resolution.
    Zone,
    /// Sub-room: advisory only, never a hard input.
    SubRoomAdvisory,
}

/// A 95 % positional error ellipse: `[major, minor, orientation]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ErrorEllipse {
    /// Semi-major axis, in metres.
    pub major_m: f32,
    /// Semi-minor axis, in metres.
    pub minor_m: f32,
    /// Orientation of the major axis, in radians, measured from the home `x`
    /// axis.
    pub orientation_rad: f32,
}

/// A confidence-scored positional result.
///
/// `position_*_m` is `None` whenever no continuous coordinate is offered — which
/// is the honest outcome of the primary estimator. When it is present, the
/// coordinate is **advisory**.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionEstimate {
    /// Which estimator produced the result.
    pub method: PositionMethod,
    /// The resolution reached.
    pub resolution: PositionResolution,
    /// The attributed room, when one separates.
    pub room: Option<RoomId>,
    /// The advisory position in the home frame, in metres.
    pub position_home_m: Option<[f32; 2]>,
    /// The advisory position in room-local metres, when the room is known.
    pub position_room_m: Option<[f32; 2]>,
    /// The 95 % error ellipse, when a coordinate is offered.
    pub error_ellipse_95_m: Option<ErrorEllipse>,
    /// Always `true` in v1: a continuous coordinate is advisory.
    pub advisory: bool,
    /// The ordered link endpoints that contributed.
    pub links_used: Vec<(NodeId, NodeId)>,
    /// The overall confidence of the estimate.
    pub confidence: Confidence,
}

/// The geometric result of a weighted least-squares multilateration.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Trilateration {
    /// The estimated position, in the frame of the input samples.
    pub position: [f32; 2],
    /// The 95 % error ellipse.
    pub error_ellipse_95: ErrorEllipse,
    /// The number of anchors used.
    pub anchors_used: usize,
    /// A confidence derived from anchor count and geometry conditioning, before
    /// any room-extent demotion.
    pub geometry_confidence: Confidence,
}

/// Per-link range sigma, from the honest path-loss bound.
#[must_use]
pub fn range_sigma_m(range_m: f32) -> f32 {
    (RANGE_SIGMA_FRACTION * range_m).max(MIN_RANGE_SIGMA_M)
}

/// Whether the anchor positions span two dimensions (are not collinear or
/// coincident).
fn anchors_spread(samples: &[[f32; 3]]) -> bool {
    let n = samples.len() as f32;
    let mx = samples.iter().map(|s| s[0]).sum::<f32>() / n;
    let my = samples.iter().map(|s| s[1]).sum::<f32>() / n;
    let (mut sxx, mut sxy, mut syy) = (0.0_f32, 0.0_f32, 0.0_f32);
    for s in samples {
        let dx = s[0] - mx;
        let dy = s[1] - my;
        sxx += dx * dx;
        sxy += dx * dy;
        syy += dy * dy;
    }
    let det = sxx * syy - sxy * sxy;
    det.is_finite() && det > 1e-9
}

/// Weighted least-squares multilateration from `[x, y, rssi_dbm]` anchor
/// samples.
///
/// Returns `None` when fewer than three anchors are given, when the anchors are
/// collinear or coincident, or when the normal equations are singular — each of
/// which is an honest "cannot locate" rather than a fabricated point. The
/// returned position is advisory.
#[must_use]
pub fn trilaterate(samples: &[[f32; 3]], model: &PathLossModel) -> Option<Trilateration> {
    let finite: Vec<[f32; 3]> = samples
        .iter()
        .copied()
        .filter(|s| s[0].is_finite() && s[1].is_finite() && s[2].is_finite())
        .collect();
    if finite.len() < 3 || !anchors_spread(&finite) {
        return None;
    }

    let ranges: Vec<f32> = finite.iter().map(|s| model.range_m(s[2])).collect();
    let sigmas: Vec<f32> = ranges.iter().map(|r| range_sigma_m(*r)).collect();
    if sigmas.iter().any(|s| !s.is_finite() || *s <= 0.0) || ranges.iter().any(|r| !r.is_finite()) {
        return None;
    }
    let weights: Vec<f32> = sigmas.iter().map(|s| 1.0 / (s * s)).collect();

    let ref_idx = (0..finite.len()).min_by(|&i, &j| {
        weights[i]
            .partial_cmp(&weights[j])
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;
    let (xr, yr) = (finite[ref_idx][0], finite[ref_idx][1]);
    let rr = ranges[ref_idx];

    let (mut a00, mut a01, mut a11, mut b0, mut b1) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
    for i in 0..finite.len() {
        if i == ref_idx {
            continue;
        }
        let h0 = 2.0 * (finite[i][0] - xr);
        let h1 = 2.0 * (finite[i][1] - yr);
        let zi = finite[i][0] * finite[i][0] - xr * xr + finite[i][1] * finite[i][1]
            - yr * yr
            - (ranges[i] * ranges[i] - rr * rr);
        let w = weights[i];
        a00 += w * h0 * h0;
        a01 += w * h0 * h1;
        a11 += w * h1 * h1;
        b0 += w * h0 * zi;
        b1 += w * h1 * zi;
    }

    let det = a00 * a11 - a01 * a01;
    if !det.is_finite() || det.abs() < 1e-9 {
        return None;
    }
    let x = (a11 * b0 - a01 * b1) / det;
    let y = (a00 * b1 - a01 * b0) / det;
    if !x.is_finite() || !y.is_finite() {
        return None;
    }

    // The covariance of the estimate is the inverse of the normal matrix.
    let (c00, c01, c11) = (a11 / det, -a01 / det, a00 / det);
    if !c00.is_finite() || !c01.is_finite() || !c11.is_finite() || c00 < 0.0 || c11 < 0.0 {
        return None;
    }
    let trace = c00 + c11;
    let disc = ((c00 - c11) * (c00 - c11) + 4.0 * c01 * c01).sqrt();
    let l_max = ((trace + disc) * 0.5).max(0.0);
    let l_min = ((trace - disc) * 0.5).max(0.0);
    let scale = CHI2_95_2DOF.sqrt();
    let major = scale * l_max.sqrt();
    let minor = scale * l_min.sqrt();
    let orientation = 0.5 * (2.0 * c01).atan2(c00 - c11);
    if !major.is_finite() || !minor.is_finite() || !orientation.is_finite() {
        return None;
    }

    let n = finite.len() as f32;
    let base = 1.0 - 1.0 / n;
    let condition = if l_max > 0.0 {
        (l_min / l_max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let geometry_confidence = unit_confidence(base * condition.sqrt());

    Some(Trilateration {
        position: [x, y],
        error_ellipse_95: ErrorEllipse {
            major_m: major,
            minor_m: minor,
            orientation_rad: orientation,
        },
        anchors_used: finite.len(),
        geometry_confidence,
    })
}
