//! Coarse-pose estimation: the deterministic, hardware-free pose path.
//!
//! This module is the in-domain surrogate for the
//! [`PoseModel`](aquila_inference::PoseModel) seam (`inference-models.md` §1).
//! It speaks the same contract type, [`Pose`](aquila_types::Pose), and is
//! wrapped by [`DspPoseModel`], which implements the seam: the core calls the
//! seam, never this estimator directly (`sister-issues-core.md` S8). The learnt
//! ONNX adapter replaces [`DspPoseModel`] behind the same seam without changing
//! the core's interface.
//!
//! # Method
//!
//! The synthetic coarse-pose fixture models each actor's torso as four
//! scatterers (left/right shoulder, left/right hip) that each micro-move
//! radially at a **distinct pose-dither frequency**, drawn per seed from the
//! shared [`TORSO_DITHER_BAND_HZ`] (`aquila-simulator`). The estimator is **not
//! told** the frequencies; it searches the band for the four dominant peaks
//! (`sister-issues-core.md` S4). For one node and one window, after removing
//! each subcarrier's temporal mean (which nulls the static-baseline leakage at a
//! non-bin frequency), a Goertzel at a candidate frequency `f` yields, per
//! subcarrier `k`, a phasor whose phase is `π − 2π·f_k·d_j/c`:
//!
//! ```text
//! G_jk = Σ_t (z_k(t) − z̄_k) · exp(−i·2π·f·(t − t0))
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
//! room), and whenever the fusion cannot form a torso. The emitted pose carries
//! [`Witness::Claimed`], not `Measured`: the deterministic surrogate is
//! validated only against synthetic fixtures, never hardware, so it is not a
//! headline claim (`evaluation.md` §5.3).

use aquila_inference::{ModelError, ModelId, PoseModel, PoseNodeSeries, PoseOutcome, PoseRequest};
use aquila_types::{
    sha256, CsiEnvelope, DetectionScope, Keypoint, KeypointId, NodeId, Pose, RoomId, SubjectId,
    Witness, KEYPOINT_COUNT, TORSO_DITHER_BAND_HZ, TORSO_DITHER_MIN_SEPARATION_HZ,
    TORSO_SCATTERER_COUNT,
};

use crate::fusion::{self, NodeTorsoRanges, TorsoGeometry};

/// The speed of light in metres per second.
const SPEED_OF_LIGHT_MPS: f64 = 299_792_458.0;

/// A full turn constant.
const TAU: f64 = core::f64::consts::TAU;

/// The default band-search step, in hertz. Fine enough to land within a quarter
/// hertz of any drawn dither frequency over the one-second analysis window.
pub const DEFAULT_DITHER_SEARCH_STEP_HZ: f64 = 0.125;

/// A pose-dither **search** specification: the estimator does not know the
/// scatterer dither frequencies; it searches `[lo_hz, hi_hz]` on `step_hz` and
/// keeps the strongest peaks at least `min_separation_hz` apart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DitherSearch {
    /// The lower band edge, in hertz.
    pub lo_hz: f64,
    /// The upper band edge, in hertz.
    pub hi_hz: f64,
    /// The search step, in hertz.
    pub step_hz: f64,
    /// The minimum separation between selected peaks, in hertz.
    pub min_separation_hz: f64,
}

impl Default for DitherSearch {
    fn default() -> Self {
        Self {
            lo_hz: TORSO_DITHER_BAND_HZ[0],
            hi_hz: TORSO_DITHER_BAND_HZ[1],
            step_hz: DEFAULT_DITHER_SEARCH_STEP_HZ,
            min_separation_hz: TORSO_DITHER_MIN_SEPARATION_HZ,
        }
    }
}

impl DitherSearch {
    /// The candidate frequencies, ascending and inclusive of both edges.
    #[must_use]
    pub fn candidates(&self) -> Vec<f64> {
        if !(self.lo_hz.is_finite()
            && self.hi_hz.is_finite()
            && self.step_hz.is_finite()
            && self.step_hz > 0.0
            && self.hi_hz >= self.lo_hz)
        {
            return Vec::new();
        }
        let count = ((self.hi_hz - self.lo_hz) / self.step_hz).floor() as usize;
        (0..=count)
            .map(|i| self.lo_hz + i as f64 * self.step_hz)
            .collect()
    }

    /// Whether the search is structurally valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.lo_hz.is_finite()
            && self.hi_hz.is_finite()
            && self.step_hz.is_finite()
            && self.step_hz > 0.0
            && self.hi_hz > self.lo_hz
            && self.min_separation_hz.is_finite()
            && self.min_separation_hz >= 0.0
    }
}

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
    search: &DitherSearch,
    min_frames: usize,
) -> Option<Pose> {
    let mut ranges: Vec<NodeTorsoRanges> = Vec::new();
    for input in nodes {
        if !input.position_m[0].is_finite() || !input.position_m[1].is_finite() {
            continue;
        }
        let Some(measured) = extract_torso_ranges(input.envelopes, search, min_frames) else {
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

/// The centred complex CSI series of a node's window, per subcarrier.
struct CentredSeries {
    /// `re[bin][frame]`, the in-phase component with the temporal mean removed.
    re: Vec<Vec<f64>>,
    /// `im[bin][frame]`, the quadrature component with the temporal mean removed.
    im: Vec<Vec<f64>>,
    /// The frame times relative to the window start, in seconds.
    times: Vec<f64>,
    /// The subcarrier count.
    bins: usize,
}

/// The four torso-scatterer ranges of one node's window, in scatterer order.
///
/// The dither frequencies are found by searching `search` for the dominant
/// peaks; `None` when the window is too short or has too few subcarriers.
/// Individual ranges that cannot be recovered are `NaN`, so the fusion treats
/// them as missing rather than as a fabricated distance.
#[must_use]
pub fn extract_torso_ranges(
    envelopes: &[CsiEnvelope],
    search: &DitherSearch,
    min_frames: usize,
) -> Option<[f64; TORSO_SCATTERER_COUNT]> {
    if envelopes.len() < min_frames.max(4) || !search.is_valid() {
        return None;
    }
    let series = centre_series(envelopes)?;
    let spacing = subcarrier_spacing_hz(envelopes)?;

    // 1. The magnitude spectrum over the search band; the torso scatterers show
    //    up as the dominant peaks.
    let candidates = search.candidates();
    let mut spectrum: Vec<(f64, f64)> = candidates
        .iter()
        .map(|&f| (f, spectrum_magnitude(&series, TAU * f)))
        .collect();
    // 2. Greedy peak selection, strongest first, enforcing the minimum
    //    separation so one scatterer is not counted twice.
    spectrum.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.total_cmp(&b.0)));
    let mut peaks: Vec<f64> = Vec::new();
    for &(f, _) in &spectrum {
        if peaks
            .iter()
            .all(|&p| (p - f).abs() >= search.min_separation_hz)
        {
            peaks.push(f);
            if peaks.len() == TORSO_SCATTERER_COUNT {
                break;
            }
        }
    }
    // Ascending frequency preserves the simulator's scatterer order (its
    // frequencies are drawn ascending across the band).
    peaks.sort_by(|a, b| a.total_cmp(b));

    let mut out = [f64::NAN; TORSO_SCATTERER_COUNT];
    for (j, &frequency) in peaks.iter().enumerate() {
        let phasors = bin_phasors(&series, TAU * frequency);
        if let Some(range) = phase_slope_range(&phasors, spacing) {
            if range.is_finite() && range >= 0.0 {
                out[j] = range;
            }
        }
    }
    Some(out)
}

/// Build the per-bin, mean-centred complex series for a window.
fn centre_series(envelopes: &[CsiEnvelope]) -> Option<CentredSeries> {
    let bins = envelopes
        .iter()
        .map(|env| env.frame.n_subcarriers())
        .min()
        .unwrap_or(0);
    if bins < 4 {
        return None;
    }
    let mut ordered: Vec<&CsiEnvelope> = envelopes.iter().collect();
    ordered.sort_by_key(|env| (env.host_rx_mono_ns, env.frame.sequence));
    let t0 = ordered[0].host_rx_mono_ns;
    let times: Vec<f64> = ordered
        .iter()
        .map(|env| (env.host_rx_mono_ns - t0) as f64 * 1e-9)
        .collect();
    let frames = ordered.len();
    let mut re = vec![vec![0.0_f64; frames]; bins];
    let mut im = vec![vec![0.0_f64; frames]; bins];
    for k in 0..bins {
        let mut mean_re = 0.0_f64;
        let mut mean_im = 0.0_f64;
        for (m, env) in ordered.iter().enumerate() {
            let (zr, zi) = complex_sample(env, k);
            re[k][m] = zr;
            im[k][m] = zi;
            mean_re += zr;
            mean_im += zi;
        }
        let mean_re = mean_re / frames as f64;
        let mean_im = mean_im / frames as f64;
        for m in 0..frames {
            re[k][m] -= mean_re;
            im[k][m] -= mean_im;
        }
    }
    Some(CentredSeries {
        re,
        im,
        times,
        bins,
    })
}

/// The total Goertzel magnitude of the centred series at angular frequency
/// `omega`: `Σ_k | Σ_t (z_k(t) − z̄_k) e^{−iωt} |`.
fn spectrum_magnitude(series: &CentredSeries, omega: f64) -> f64 {
    let mut total = 0.0_f64;
    for k in 0..series.bins {
        let (re, im) = goertzel(series, k, omega);
        total += (re * re + im * im).sqrt();
    }
    total
}

/// The per-bin Goertzel phasors of the centred series at `omega`.
fn bin_phasors(series: &CentredSeries, omega: f64) -> Vec<(f64, f64)> {
    (0..series.bins)
        .map(|k| goertzel(series, k, omega))
        .collect()
}

/// One subcarrier's Goertzel sum `Σ_t (z_k(t) − z̄_k) e^{−iωt}`.
fn goertzel(series: &CentredSeries, k: usize, omega: f64) -> (f64, f64) {
    let bin_re = &series.re[k];
    let bin_im = &series.im[k];
    let mut acc_re = 0.0_f64;
    let mut acc_im = 0.0_f64;
    for (t, (&zr, &zi)) in series.times.iter().zip(bin_re.iter().zip(bin_im)) {
        let angle = omega * t;
        let (sin_a, cos_a) = angle.sin_cos();
        // z · exp(-i·angle)
        acc_re += zr * cos_a + zi * sin_a;
        acc_im += zi * cos_a - zr * sin_a;
    }
    (acc_re, acc_im)
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
fn subcarrier_spacing_hz(envelopes: &[CsiEnvelope]) -> Option<f64> {
    let frame = &envelopes.first()?.frame;
    let bins = frame.n_subcarriers();
    if bins == 0 || frame.bandwidth_mhz == 0 {
        return None;
    }
    let bandwidth_hz = f64::from(frame.bandwidth_mhz) * 1e6;
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
        // The deterministic surrogate is validated only against synthetic
        // fixtures, never hardware, so it is a `Claimed` pose, not a
        // hardware-`Measured` one (`evaluation.md` §5.3, `sister-issues-core.md`
        // S4).
        witness: Witness::Claimed,
    }
}

/// The deterministic DSP coarse-pose adapter behind the `PoseModel` seam.
///
/// This is the in-core surrogate that lets the core call the seam without
/// moving the DSP out of `aquila-sensing` (`sister-issues-core.md` S8). It
/// consumes the raw per-node series on a [`PoseRequest`]; a learnt ONNX adapter
/// would consume the packed features instead.
pub struct DspPoseModel {
    id: ModelId,
    geometry: TorsoGeometry,
    search: DitherSearch,
    min_frames: usize,
}

impl DspPoseModel {
    /// Construct the adapter with a fixed geometry and dither search.
    #[must_use]
    pub fn new(geometry: TorsoGeometry, search: DitherSearch, min_frames: usize) -> Self {
        Self {
            // A content-addressed identity for the deterministic method, so a
            // caller can distinguish it from any learnt weights.
            id: ModelId::from_digest(sha256(b"aquila.dsp.pose.v1")),
            geometry,
            search,
            min_frames,
        }
    }
}

impl PoseModel for DspPoseModel {
    fn id(&self) -> &ModelId {
        &self.id
    }

    fn infer(&self, requests: &[PoseRequest]) -> Result<Vec<PoseOutcome>, ModelError> {
        if requests.is_empty() {
            return Err(ModelError::EmptyBatch);
        }
        Ok(requests
            .iter()
            .map(|request| self.infer_one(request))
            .collect())
    }
}

impl DspPoseModel {
    /// Serve one request: no pose for a non-room scope or a request without a
    /// raw series.
    fn infer_one(&self, request: &PoseRequest) -> PoseOutcome {
        let DetectionScope::Room(room) = &request.scope else {
            return PoseOutcome {
                pose: None,
                presence: None,
                count: None,
            };
        };
        if request.series.is_empty() {
            return PoseOutcome {
                pose: None,
                presence: None,
                count: None,
            };
        }
        let inputs: Vec<PoseNodeInput<'_>> = request
            .series
            .iter()
            .map(|series: &PoseNodeSeries| PoseNodeInput {
                node: series.node,
                position_m: series.position_m,
                weight: series.weight,
                envelopes: &series.envelopes,
            })
            .collect();
        let subject = request.subject.unwrap_or(SubjectId::new(0));
        let pose = estimate_room_pose(
            room,
            subject,
            request.span.start_unix_ms,
            &inputs,
            &self.geometry,
            &self.search,
            self.min_frames,
        );
        PoseOutcome {
            pose,
            presence: None,
            count: None,
        }
    }
}
