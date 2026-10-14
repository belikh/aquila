//! Coarse-pose estimation: the deterministic, hardware-free pose path.
//!
//! This module is the in-domain surrogate for the [`PoseModel`](aquila_inference::PoseModel)
//! seam (`inference-models.md` §1). It speaks the same contract type,
//! [`Pose`](aquila_types::Pose), and its outputs are `Witness::Measured`. The
//! learned ONNX adapter will replace it behind the same seam without changing
//! the core's interface; until then the core calls this estimator directly,
//! because the seam's `FeatureVector` layout is not yet pinned
//! (`inference-models.md` §8.3) and this path needs the raw per-node CSI series.
//!
//! # Method
//!
//! The synthetic coarse-pose fixture models each actor's torso as four
//! scatterers (left/right shoulder, left/right hip) that each micro-move
//! radially at a **distinct pose-dither frequency** (`aquila-simulator`'s
//! `TORSO_DITHER_HZ`). For one node and one window, a Goertzel of the complex
//! CSI at dither frequency `f_j` yields, per subcarrier `k`, a phasor whose
//! phase is `π − 2π·f_k·d_j/c`:
//!
//! ```text
//! G_jk = Σ_t  z_k(t) · exp(−i·2π·f_j·(t − t0))
//! arg(G_jk) = π − 2π·f_k·d_j/c       (to first order in the dither)
//! ```
//!
//! The phase slope across subcarriers depends only on the subcarrier spacing
//! `s` (`arg` changes by `−2π·s·d_j/c` per subcarrier), so a weighted
//! least-squares line fit recovers the scatterer's range `d_j` **without** the
//! centre-frequency phase ambiguity that defeats naive delay ranging: only the
//! slope is used, and the fine `c/f_centre` lattice cancels. `d_j` is the
//! keypoint's range to the node.
//!
//! The per-node ranges are then combined by
//! [`crate::fusion::fuse_torso_ranges`] into a rigid torso pose — feature-level
//! fusion: scalar range features are combined across nodes, never raw phase
//! (`fusion-sync.md` §2, §3).
//!
//! # Never fabricate
//!
//! Absence is expressed through the contract: no estimate means `None`, never a
//! zeroed or guessed pose. The estimator returns `None` for an empty node or
//! window, for a single-node view (a view-relative pose cannot be placed in the
//! room), and whenever the fusion cannot form a torso.

use aquila_types::{
    CsiEnvelope, Keypoint, KeypointId, NodeId, Pose, RoomId, SubjectId, Witness, KEYPOINT_COUNT,
};

use crate::fusion::{self, NodeTorsoRanges, TorsoGeometry, TORSO_KEYPOINT_SCATTERS};

/// The speed of light in metres per second.
const SPEED_OF_LIGHT_MPS: f64 = 299_792_458.0;

/// The default per-scatterer pose-dither frequencies, in hertz, in torso
/// scatterer order (left shoulder, right shoulder, left hip, right hip).
///
/// This must match the synthetic pose signature the simulator injects
/// (`aquila_simulator`'s `TORSO_DITHER_HZ`); it is duplicated here because the
/// sensing crate may not depend on the simulator in the dependency DAG. A
/// future learned `PoseModel` does not need it at all.
pub const DEFAULT_TORSO_DITHER_HZ: [f64; TORSO_KEYPOINT_SCATTERS] = [3.0, 5.0, 7.0, 9.0];

/// A full turn constant.
const TAU: f64 = core::f64::consts::TAU;

/// The COCO-17 keypoint order, so an assembled pose's array is self-consistent.
const KEYPOINT_ORDER: [KeypointId; KEYPOINT_COUNT] = [
    KeypointId::Nose,
    KeypointId::LeftEye,
    KeypointId::RightEye,
    KeypointId::LeftEar,
    KeypointId::RightEar,
    KeypointId::LeftShoulder,
    KeypointId::RightShoulder,
    KeypointId::LeftElbow,
    KeypointId::RightElbow,
    KeypointId::LeftWrist,
    KeypointId::RightWrist,
    KeypointId::LeftHip,
    KeypointId::RightHip,
    KeypointId::LeftKnee,
    KeypointId::RightKnee,
    KeypointId::LeftAnkle,
    KeypointId::RightAnkle,
];

/// One node's input to coarse-pose estimation.
#[derive(Debug, Clone, Copy)]
pub struct PoseNodeInput<'a> {
    /// The contributing node.
    pub node: NodeId,
    /// The node's room-local planar position, in metres.
    pub position_m: [f64; 2],
    /// The node's fusion weight in `[0, 1]`.
    pub weight: f64,
    /// The node's CSI envelopes for the window, unordered.
    pub envelopes: &'a [CsiEnvelope],
}

/// Estimate one room's coarse pose from its nodes' windowed CSI.
///
/// Returns `None` — never a fabricated pose — when no torso can be formed: an
/// empty or too-short window, an unplaced node, a single-node view, or a fit
/// with no usable ranges.
#[must_use]
pub fn estimate_room_pose(
    room: &RoomId,
    subject: SubjectId,
    t_unix_ms: i64,
    nodes: &[PoseNodeInput<'_>],
    geometry: &TorsoGeometry,
    dither_hz: &[f64; TORSO_KEYPOINT_SCATTERS],
    min_frames: usize,
) -> Option<Pose> {
    let mut ranges: Vec<NodeTorsoRanges> = Vec::new();
    for input in nodes {
        if !input.position_m[0].is_finite() || !input.position_m[1].is_finite() {
            continue;
        }
        let Some(measured) = extract_torso_ranges(input.envelopes, dither_hz, min_frames) else {
            continue;
        };
        ranges.push(NodeTorsoRanges {
            node: input.node,
            position_m: input.position_m,
            ranges_m: measured,
            weight: input.weight,
        });
    }
    let estimate = fusion::fuse_torso_ranges(&ranges, geometry)?;
    if !estimate.confidence.is_finite() {
        return None;
    }
    Some(assemble_pose(room, subject, t_unix_ms, &estimate, geometry))
}

/// The four torso-scatterer ranges of one node's window, in scatterer order.
///
/// `None` when the window is too short or has too few subcarriers. Individual
/// ranges that cannot be recovered are `NaN`, so the fusion treats them as
/// missing rather than as a fabricated distance.
#[must_use]
pub fn extract_torso_ranges(
    envelopes: &[CsiEnvelope],
    dither_hz: &[f64; TORSO_KEYPOINT_SCATTERS],
    min_frames: usize,
) -> Option<[f64; TORSO_KEYPOINT_SCATTERS]> {
    if envelopes.len() < min_frames.max(4) {
        return None;
    }
    let bins = envelopes
        .iter()
        .map(|env| env.frame.n_subcarriers())
        .min()
        .unwrap_or(0);
    if bins < 4 {
        return None;
    }

    // Time-ordered samples, relative to the window's first arrival.
    let mut ordered: Vec<&CsiEnvelope> = envelopes.iter().collect();
    ordered.sort_by_key(|env| (env.host_rx_mono_ns, env.frame.sequence));
    let t0 = ordered[0].host_rx_mono_ns;
    let times: Vec<f64> = ordered
        .iter()
        .map(|env| (env.host_rx_mono_ns - t0) as f64 * 1e-9)
        .collect();
    let spacing = subcarrier_spacing_hz(ordered[0])?;

    let mut out = [f64::NAN; TORSO_KEYPOINT_SCATTERS];
    for (j, &frequency) in dither_hz.iter().enumerate() {
        if !frequency.is_finite() || frequency <= 0.0 {
            continue;
        }
        let omega = TAU * frequency;
        // Per-subcarrier Goertzel at the dither frequency.
        let mut phasors: Vec<(f64, f64)> = Vec::with_capacity(bins);
        for k in 0..bins {
            let mut re = 0.0_f64;
            let mut im = 0.0_f64;
            for (sample, time) in ordered.iter().zip(&times) {
                let (zr, zi) = complex_sample(sample, k);
                let angle = omega * time;
                let (sin_a, cos_a) = angle.sin_cos();
                // z · exp(-i·angle)
                re += zr * cos_a + zi * sin_a;
                im += zi * cos_a - zr * sin_a;
            }
            phasors.push((re, im));
        }
        if let Some(range) = phase_slope_range(&phasors, spacing) {
            if range.is_finite() && range >= 0.0 {
                out[j] = range;
            }
        }
    }
    Some(out)
}

/// The complex CSI sample of one subcarrier, the raw int8 `(I, Q)` pair.
fn complex_sample(envelope: &CsiEnvelope, k: usize) -> (f64, f64) {
    (
        f64::from(envelope.frame.i_at(k)),
        f64::from(envelope.frame.q_at(k)),
    )
}

/// The subcarrier spacing of a frame, in hertz, from its bandwidth and bin
/// count.
fn subcarrier_spacing_hz(envelope: &CsiEnvelope) -> Option<f64> {
    let bins = envelope.frame.n_subcarriers();
    if bins == 0 || envelope.frame.bandwidth_mhz == 0 {
        return None;
    }
    let bandwidth_hz = f64::from(envelope.frame.bandwidth_mhz) * 1e6;
    let spacing = bandwidth_hz / bins as f64;
    (spacing.is_finite() && spacing > 0.0).then_some(spacing)
}

/// Recover a scatterer's range from the across-subcarrier phase slope of its
/// dither phasors.
///
/// The dither phasor at subcarrier `k` has phase `π − 2π·f_k·d/c`, so its slope
/// against the subcarrier index is `−2π·s·d/c`. Unwrapping is safe because the
/// per-subcarrier step is far below `π` for any room-scale range, and only the
/// slope is used, so the centre-frequency phase cancels.
fn phase_slope_range(phasors: &[(f64, f64)], spacing: f64) -> Option<f64> {
    let n = phasors.len();
    if n < 4 || !spacing.is_finite() || spacing <= 0.0 {
        return None;
    }
    let mut theta = vec![0.0_f64; n];
    for (k, &(re, im)) in phasors.iter().enumerate() {
        if re == 0.0 && im == 0.0 {
            return None;
        }
        theta[k] = im.atan2(re);
    }
    for k in 1..n {
        let step = theta[k] - theta[k - 1];
        theta[k] = theta[k - 1] + wrap_to_pi(step);
    }

    // Magnitude-weighted least-squares slope against the subcarrier index.
    let (mut sw, mut sx, mut sy, mut sxx, mut sxy) = (0.0_f64, 0.0, 0.0, 0.0, 0.0);
    for (k, &(re, im)) in phasors.iter().enumerate() {
        let weight = (re * re + im * im).sqrt();
        let x = k as f64;
        let y = theta[k];
        sw += weight;
        sx += weight * x;
        sy += weight * y;
        sxx += weight * x * x;
        sxy += weight * x * y;
    }
    let denom = sw * sxx - sx * sx;
    if !denom.is_finite() || denom.abs() < 1e-12 {
        return None;
    }
    let slope = (sw * sxy - sx * sy) / denom;
    let range = -slope * SPEED_OF_LIGHT_MPS / (TAU * spacing);
    range.is_finite().then_some(range)
}

/// Wrap an angle into `(-π, π]`.
fn wrap_to_pi(x: f64) -> f64 {
    x - TAU * (x / TAU).round()
}

/// A COCO-17 keypoint array with every keypoint honestly absent.
fn blank_keypoints() -> [Keypoint; KEYPOINT_COUNT] {
    let mut keys = [Keypoint {
        id: KeypointId::Nose,
        x_m: f32::NAN,
        y_m: f32::NAN,
        z_m: None,
        confidence: 0.0,
    }; KEYPOINT_COUNT];
    for (key, id) in keys.iter_mut().zip(KEYPOINT_ORDER) {
        key.id = id;
    }
    keys
}

/// Assemble a contract [`Pose`] from a fused torso estimate.
fn assemble_pose(
    room: &RoomId,
    subject: SubjectId,
    t_unix_ms: i64,
    estimate: &fusion::TorsoEstimate,
    geometry: &TorsoGeometry,
) -> Pose {
    let keypoints = geometry.keypoints_m(estimate.centre_m, estimate.yaw_rad);
    let confidence = estimate.confidence.clamp(0.0, 1.0) as f32;
    let mut keys = blank_keypoints();
    for (slot, id) in [
        (0usize, KeypointId::LeftShoulder),
        (1, KeypointId::RightShoulder),
        (2, KeypointId::LeftHip),
        (3, KeypointId::RightHip),
    ] {
        keys[id as usize] = Keypoint {
            id,
            x_m: keypoints[slot][0] as f32,
            y_m: keypoints[slot][1] as f32,
            z_m: None,
            confidence,
        };
    }
    Pose {
        subject,
        room: room.clone(),
        t_unix_ms,
        keys,
        score: confidence,
        witness: Witness::Measured,
    }
}
