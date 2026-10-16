//! Feature-level fusion.
//!
//! Fusion combines per-node features and detections only; there is no cross-node
//! raw-phase combination (`fusion-sync.md` §2). Presence uses **log-odds
//! pooling** — a commutative, associative Bayesian product of independent
//! evidence — with each node's contribution clamped so no single node can force
//! occupancy. Count uses a **weighted arithmetic mixture** of node PMFs, which
//! avoids the over-confidence of pooling counts as independent decisions.
//!
//! Unknown is never conflated with absent: a prior log-odds of `0` yields
//! `p = 0.5`, and an empty evidence set yields the prior, never "absent".

use aquila_types::{NodeId, ZoneId};

/// A discrete probability mass function over counts `0..=MAX_COUNT_V1`.
#[derive(Debug, Clone, PartialEq)]
pub struct CountPmf {
    /// The per-count probabilities; `probs[0]` is `P(0)`, and so on.
    pub probs: Vec<f64>,
}

impl CountPmf {
    /// A point mass at `count`, clamped into range.
    #[must_use]
    pub fn point(count: u8, max: usize) -> Self {
        let mut probs = vec![0.0; max + 1];
        let index = usize::from(count).min(max);
        probs[index] = 1.0;
        Self { probs }
    }

    /// The MAP count and its probability.
    #[must_use]
    pub fn map(&self) -> (u8, f64) {
        let mut best_index = 0usize;
        let mut best = f64::NEG_INFINITY;
        for (i, p) in self.probs.iter().enumerate() {
            if *p > best {
                best = *p;
                best_index = i;
            }
        }
        (best_index.min(u8::MAX as usize) as u8, best.max(0.0))
    }

    /// Renormalise the mass to sum to one, guarding against an all-zero PMF.
    ///
    /// Visible within the crate so the escalation step's convolution
    /// (`escalation::convolve`) can renormalise a truncated count PMF.
    pub(crate) fn normalised(mut self) -> Self {
        let sum: f64 = self.probs.iter().sum();
        if sum > 0.0 && sum.is_finite() {
            for p in &mut self.probs {
                *p /= sum;
            }
        }
        self
    }
}

/// One node's evidence for one window (the v1 subset of `fusion-sync.md` §2.1).
#[derive(Debug, Clone, PartialEq)]
pub struct NodeEvidence {
    /// The contributing node.
    pub node: NodeId,
    /// The zone the node contributes to.
    pub zone: ZoneId,
    /// The log-odds of presence in this node's volume.
    pub presence_logit: f64,
    /// The node's count PMF over its volume.
    pub count_pmf: CountPmf,
    /// The fusion weight, in `[0, 1]`.
    pub weight: f64,
    /// The number of frames behind the evidence.
    pub frames: usize,
}

/// The logistic function, evaluated without overflowing for large magnitudes.
#[must_use]
pub fn sigmoid(x: f64) -> f64 {
    if x >= 0.0 {
        let z = (-x).exp();
        1.0 / (1.0 + z)
    } else {
        let z = x.exp();
        z / (1.0 + z)
    }
}

/// Clamp a log-odds contribution to `[-l_max, l_max]`.
#[must_use]
pub fn clamp_logit(x: f64, l_max: f64) -> f64 {
    if !x.is_finite() {
        0.0
    } else {
        x.clamp(-l_max, l_max)
    }
}

/// Pool per-node presence evidence into a room log-odds.
///
/// `L = prior + Σ w_n · clamp(L_n)`. The sum is over a canonical node order so
/// the result is invariant to the order evidence is supplied in.
#[must_use]
pub fn pool_presence_logit(prior_logit: f64, evidence: &[NodeEvidence], l_max: f64) -> f64 {
    let mut ordered: Vec<&NodeEvidence> = evidence.iter().collect();
    ordered.sort_by_key(|e| e.node);
    let mut logit = prior_logit;
    for e in ordered {
        let weight = e.weight.clamp(0.0, 1.0);
        logit += weight * clamp_logit(e.presence_logit, l_max);
    }
    logit
}

/// The pooled presence probability of a room.
#[must_use]
pub fn pooled_presence_probability(prior_logit: f64, evidence: &[NodeEvidence], l_max: f64) -> f64 {
    sigmoid(pool_presence_logit(prior_logit, evidence, l_max))
}

/// A two-point PMF for one node: `P(0) = 1 - p`, `P(1) = p`.
///
/// A unary node cannot see two people; more than one occupant in a room is the
/// room mixture's job, not a single node's.
#[must_use]
pub fn node_count_pmf(presence_probability: f64, max: usize) -> CountPmf {
    let p = presence_probability.clamp(0.0, 1.0);
    let mut probs = vec![0.0; max + 1];
    probs[0] = 1.0 - p;
    if max >= 1 {
        probs[1] = p;
    } else {
        probs[0] = 1.0;
    }
    CountPmf { probs }
}

/// Mix node count PMFs by weight into one room PMF.
///
/// Nodes within a room are mixed, not summed, so two nodes seeing the same
/// person do not double-count (`fusion-sync.md` §2.2).
#[must_use]
pub fn mix_count_pmf(evidence: &[NodeEvidence], max: usize) -> CountPmf {
    let mut ordered: Vec<&NodeEvidence> = evidence.iter().collect();
    ordered.sort_by_key(|e| e.node);
    let total_weight: f64 = ordered.iter().map(|e| e.weight.clamp(0.0, 1.0)).sum();
    if ordered.is_empty() || total_weight <= 0.0 {
        // No usable evidence: a broad, uninformative PMF, never a fabricated 0.
        let uniform = 1.0 / (max + 1) as f64;
        return CountPmf {
            probs: vec![uniform; max + 1],
        };
    }
    let mut probs = vec![0.0_f64; max + 1];
    for e in ordered {
        let weight = e.weight.clamp(0.0, 1.0) / total_weight;
        for (slot, p) in probs.iter_mut().zip(e.count_pmf.probs.iter()) {
            *slot += weight * p;
        }
    }
    CountPmf { probs }.normalised()
}

/// The per-node fusion weight `w_n = q_n · h_n` (`fusion-sync.md` §2.3).
///
/// `q_n` is the node's global-calibration quality in `[0, 1]` (the calibration
/// author's contract); `h_n` is the node's clock/health factor in `[0, 1]`
/// (`Good → 1.0`, degrading with drift and packet loss). Both are clamped, so a
/// node with `q_n = 0` contributes nothing and a node with no health evidence
/// weighs by its calibration quality alone.
#[must_use]
pub fn node_fusion_weight(calibration_quality: f64, clock_health: f64) -> f64 {
    let q = if calibration_quality.is_finite() {
        calibration_quality.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let h = if clock_health.is_finite() {
        clock_health.clamp(0.0, 1.0)
    } else {
        0.0
    };
    q * h
}

/// The number of torso scatterer keypoints the coarse-pose model places, in
/// scatterer order: left shoulder, right shoulder, left hip, right hip.
/// Re-exported from the level-0 contract crate so the simulator, fusion and the
/// estimator share one declaration (`sister-issues-core.md` S7).
pub const TORSO_KEYPOINT_SCATTERS: usize = aquila_types::TORSO_SCATTERER_COUNT;

/// The rigid planar torso geometry the coarse-pose fusion fits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TorsoGeometry {
    /// Half the shoulder-to-shoulder width, in metres.
    pub shoulder_half_width_m: f64,
    /// Half the hip-to-hip width, in metres.
    pub hip_half_width_m: f64,
    /// The neck-to-pelvis torso span, in metres.
    pub torso_length_m: f64,
}

impl TorsoGeometry {
    /// A default adult torso: 0.40 m shoulders, 0.24 m hips, 0.50 m span.
    #[must_use]
    pub const fn adult() -> Self {
        Self {
            shoulder_half_width_m: 0.20,
            hip_half_width_m: 0.12,
            torso_length_m: 0.50,
        }
    }

    /// The four keypoint offsets in the body-local plane, in scatterer order.
    ///
    /// The pelvis is the body origin; the neck is `torso_length` forward; the
    /// shoulders straddle the neck and the hips straddle the pelvis.
    #[must_use]
    pub fn local_offsets(&self) -> [[f64; 2]; TORSO_KEYPOINT_SCATTERS] {
        [
            [self.torso_length_m, self.shoulder_half_width_m],
            [self.torso_length_m, -self.shoulder_half_width_m],
            [0.0, self.hip_half_width_m],
            [0.0, -self.hip_half_width_m],
        ]
    }

    /// The four torso keypoint positions for a body centred at `centre` with
    /// facing `yaw`, in metres.
    #[must_use]
    pub fn keypoints_m(&self, centre: [f64; 2], yaw: f64) -> [[f64; 2]; TORSO_KEYPOINT_SCATTERS] {
        let (sin_y, cos_y) = yaw.sin_cos();
        self.local_offsets().map(|[f, l]| {
            [
                centre[0] + f * cos_y - l * sin_y,
                centre[1] + f * sin_y + l * cos_y,
            ]
        })
    }
}

/// One node's measured torso-scatterer ranges and its fusion weight, for the
/// coarse-pose combination.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeTorsoRanges {
    /// The contributing node.
    pub node: NodeId,
    /// The node's room-local planar position, in metres.
    pub position_m: [f64; 2],
    /// The measured range to each torso scatterer, in metres, in scatterer
    /// order. A non-finite or non-positive entry is treated as missing.
    pub ranges_m: [f64; TORSO_KEYPOINT_SCATTERS],
    /// The node's fusion weight in `[0, 1]` ([`node_fusion_weight`]).
    pub weight: f64,
}

/// A fused coarse-pose torso estimate.
#[derive(Debug, Clone, PartialEq)]
pub struct TorsoEstimate {
    /// The torso centre (pelvis) in room-local metres.
    pub centre_m: [f64; 2],
    /// The facing yaw in radians.
    pub yaw_rad: f64,
    /// The RMS range residual of the fit, in metres.
    pub residual_rms_m: f64,
    /// A confidence in `[0, 1]` derived from the residual and the node weights.
    pub confidence: f64,
    /// The contributing node ids, canonical-sorted.
    pub contributors: Vec<NodeId>,
}

/// Fuse per-node torso-scatterer ranges into one rigid coarse-pose torso
/// (`fusion-sync.md` §2.5 "pose", Slice 4).
///
/// The combination is **feature-level**: each node contributes scalar range
/// features derived from its own CSI, and no cross-node raw phase is combined.
/// The four scatterers are identified across nodes by their pose-dither
/// frequency, so a scatterer's range at one node corresponds to the same
/// keypoint at the other.
///
/// Because two range measurements to two nodes leave an exact mirror ambiguity
/// across the node line (the room has no third anchor in v1), the fit carries a
/// **room-local positive-quadrant prior**: a coarse interior assumption that a
/// person stands inside the room's local `x ≥ 0, y ≥ 0` quadrant. This is a
/// stated prior, not a measurement; a third node or a room polygon removes it.
///
/// Returns `None` when fewer than two nodes carry a usable range, so no pose is
/// fabricated from a single view (`fusion-sync.md` §2.2, "Coarse pose from
/// uncoherent views is view-relative").
#[must_use]
pub fn fuse_torso_ranges(
    nodes: &[NodeTorsoRanges],
    geometry: &TorsoGeometry,
) -> Option<TorsoEstimate> {
    let usable: Vec<&NodeTorsoRanges> = nodes
        .iter()
        .filter(|n| n.weight > 0.0 && n.ranges_m.iter().filter(|r| valid_range(r)).count() > 0)
        .collect();
    if usable.len() < 2 {
        return None;
    }
    let valid_total: usize = usable
        .iter()
        .map(|n| n.ranges_m.iter().filter(|r| valid_range(r)).count())
        .sum();
    if valid_total < TORSO_KEYPOINT_SCATTERS {
        return None;
    }

    // Search bounds: the node centroid plus a generous margin.
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for n in &usable {
        min_x = min_x.min(n.position_m[0]);
        max_x = max_x.max(n.position_m[0]);
        min_y = min_y.min(n.position_m[1]);
        max_y = max_y.max(n.position_m[1]);
    }
    let margin = 4.0_f64;
    let x0 = min_x - margin;
    let x1 = max_x + margin;
    let y0 = min_y - margin;
    let y1 = max_y + margin;

    let mut best = Fit {
        cost: f64::INFINITY,
        centre: [0.5 * (x0 + x1), 0.5 * (y0 + y1)],
        yaw: 0.0,
    };
    // Coarse pass, then two local refinements. Each pass narrows the centre
    // window and the yaw window around the previous best.
    let mut step = 0.25_f64;
    let mut span = [x1 - x0, y1 - y0];
    let mut origin = (x0, y0);
    let mut yaw_span = TAU;
    let mut yaw_origin = 0.0_f64;
    for pass in 0..3 {
        let nx = ((span[0] / step).ceil() as usize).max(1) + 1;
        let ny = ((span[1] / step).ceil() as usize).max(1) + 1;
        let yaw_steps = if pass == 0 { 24 } else { 12 };
        let mut yaw_step = yaw_span / yaw_steps as f64;
        for iy in 0..ny {
            for ix in 0..nx {
                let centre = [origin.0 + ix as f64 * step, origin.1 + iy as f64 * step];
                for iyaw in 0..=yaw_steps {
                    let yaw = yaw_origin + iyaw as f64 * yaw_step;
                    let cost = fit_cost(centre, yaw, &usable, geometry);
                    if cost < best.cost {
                        best = Fit { cost, centre, yaw };
                    }
                }
            }
        }
        // Refine around the best estimate.
        step /= 5.0;
        span = [step * 4.0, step * 4.0];
        origin = (best.centre[0] - step * 2.0, best.centre[1] - step * 2.0);
        yaw_step = yaw_span / yaw_steps as f64;
        yaw_span = yaw_step * 4.0;
        yaw_origin = best.yaw - yaw_step * 2.0;
    }

    let (residual_rms, weight_sum, weight_norm) = residual_stats(&best, &usable, geometry);
    let mean_weight = if weight_norm > 0.0 {
        weight_sum / weight_norm
    } else {
        0.0
    };
    let residual_term = 1.0 / (1.0 + residual_rms / 0.10);
    let confidence = (residual_term * mean_weight).clamp(0.0, 1.0);
    let mut contributors: Vec<NodeId> = usable.iter().map(|n| n.node).collect();
    contributors.sort_unstable();
    contributors.dedup();
    Some(TorsoEstimate {
        centre_m: best.centre,
        yaw_rad: best.yaw,
        residual_rms_m: residual_rms,
        confidence,
        contributors,
    })
}

/// A candidate torso fit and its cost.
struct Fit {
    cost: f64,
    centre: [f64; 2],
    yaw: f64,
}

/// Whether a measured range is usable.
fn valid_range(r: &f64) -> bool {
    r.is_finite() && *r > 0.0
}

/// The weighted sum of squared range residuals plus the interior prior.
fn fit_cost(
    centre: [f64; 2],
    yaw: f64,
    nodes: &[&NodeTorsoRanges],
    geometry: &TorsoGeometry,
) -> f64 {
    let keypoints = geometry.keypoints_m(centre, yaw);
    let mut cost = 0.0_f64;
    for n in nodes {
        let weight = n.weight.clamp(0.0, 1.0);
        for (j, keypoint) in keypoints.iter().enumerate() {
            if !valid_range(&n.ranges_m[j]) {
                continue;
            }
            let dx = keypoint[0] - n.position_m[0];
            let dy = keypoint[1] - n.position_m[1];
            let predicted = (dx * dx + dy * dy).sqrt();
            let residual = predicted - n.ranges_m[j];
            cost += weight * residual * residual;
        }
    }
    // Room-local positive-quadrant interior prior (see the function docs).
    for keypoint in &keypoints {
        let px = (-keypoint[0]).max(0.0);
        let py = (-keypoint[1]).max(0.0);
        cost += 4.0 * (px * px + py * py);
    }
    cost
}

/// The RMS residual, and weight sums, of a candidate fit over the valid ranges.
fn residual_stats(
    fit: &Fit,
    nodes: &[&NodeTorsoRanges],
    geometry: &TorsoGeometry,
) -> (f64, f64, f64) {
    let keypoints = geometry.keypoints_m(fit.centre, fit.yaw);
    let mut sum_sq = 0.0_f64;
    let mut weight_sum = 0.0_f64;
    let mut weight_norm = 0.0_f64;
    for n in nodes {
        let weight = n.weight.clamp(0.0, 1.0);
        weight_norm += weight;
        for (j, keypoint) in keypoints.iter().enumerate() {
            if !valid_range(&n.ranges_m[j]) {
                continue;
            }
            let dx = keypoint[0] - n.position_m[0];
            let dy = keypoint[1] - n.position_m[1];
            let predicted = (dx * dx + dy * dy).sqrt();
            let residual = predicted - n.ranges_m[j];
            sum_sq += weight * residual * residual;
            weight_sum += weight;
        }
    }
    let rms = if weight_sum > 0.0 {
        (sum_sq / weight_sum).sqrt()
    } else {
        f64::INFINITY
    };
    (rms, weight_sum, weight_norm)
}

/// A full turn constant.
const TAU: f64 = core::f64::consts::TAU;
