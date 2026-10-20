//! Reference pipeline composition.
//!
//! [`process`] runs the stages of `signal.md` §5 in order and assembles
//! [`Features`]. It is the thin top-level entry point: decode is performed by
//! the caller (the chunk is already typed), and every stage below is pure.
//! `select_subcarriers` would run at calibration time; here it is applied
//! deterministically to the chunk's motion energies so the reference path stays
//! self-contained.

use aquila_types::{FeatureVector, NodeId, VitalsStatus};

use crate::decode::{ComplexFrame, SubcarrierLayout};
use crate::error::{ConfigError, SignalError};
use crate::filter::{bandpass, Band, BiquadCoeffs};
use crate::math::atan2_approx;
use crate::motion::{motion_features, rms};
use crate::normalise::{normalise, Calibration, CalibrationState};
use crate::phase::{sanitise_phase, unwrap_phase};
use crate::resample::resample_uniform;
use crate::selection::{select_subcarriers, SubcarrierFeatureMatrix};
use crate::spectrogram::{bvp_track, stft, BvpSample, StftConfig};
use crate::vitals::{estimate_vitals, VitalEstimate, VitalsConfig, VitalsEstimates, VitalsParams};

/// A sequence anomaly surfaced from the transport, never enforced destructively
/// (`signal.md` §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeqAnomaly {
    /// A forward jump: frames `from+1 ..= to-1` were lost.
    Gap {
        /// The last sequence number seen before the gap.
        from: u32,
        /// The next sequence number seen.
        to: u32,
    },
    /// A backward jump across a gap.
    Reorder {
        /// The sequence number expected next.
        expected: u32,
        /// The sequence number found instead.
        found: u32,
    },
    /// A single-step backward jump.
    Regression {
        /// The previous sequence number.
        previous: u32,
        /// The current sequence number.
        current: u32,
    },
    /// A repeated sequence number.
    Duplicate(u32),
}

/// Surface transport reality (loss, reordering, reboot) as data.
///
/// A pure function of the `seq` fields (`S1`); never errors and never drops
/// frames. A monotone, contiguous run produces an empty list (`S2`); gaps are
/// expected and tolerated (`S3`).
#[must_use]
pub fn validate_sequence(frames: &[ComplexFrame]) -> Vec<SeqAnomaly> {
    let seqs: Vec<u32> = frames.iter().map(|f| f.meta.seq).collect();
    anomalies_for(&seqs)
}

/// The anomaly scan over already-extracted sequence numbers.
fn anomalies_for(seqs: &[u32]) -> Vec<SeqAnomaly> {
    let mut out = Vec::new();
    let mut previous: Option<u32> = None;
    for &seq in seqs {
        if let Some(prev) = previous {
            if seq == prev {
                out.push(SeqAnomaly::Duplicate(seq));
            } else if seq < prev {
                if seq + 1 == prev {
                    out.push(SeqAnomaly::Regression {
                        previous: prev,
                        current: seq,
                    });
                } else {
                    out.push(SeqAnomaly::Reorder {
                        expected: prev + 1,
                        found: seq,
                    });
                }
            } else if seq > prev + 1 {
                out.push(SeqAnomaly::Gap {
                    from: prev,
                    to: seq,
                });
            }
        }
        previous = Some(seq);
    }
    out
}

/// The single frozen reference configuration (`signal.md` §2).
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// The subcarrier grid.
    pub layout: SubcarrierLayout,
    /// The breathing band; default `0.10..0.60 Hz`.
    pub band_breathing: Band,
    /// The heart band; default `0.80..2.50 Hz`.
    pub band_heart: Band,
    /// The motion band; default `0.10..5.00 Hz`.
    pub band_motion: Band,
    /// The uniform resample grid for the STFT, in hertz.
    pub uniform_fs_hz: f32,
    /// The STFT window length, in samples.
    pub stft_window: usize,
    /// The STFT hop, in samples.
    pub stft_hop: usize,
    /// The number of subcarriers to select.
    pub select_k: usize,
    /// The vitals-estimation parameters (bands, search step, SNR floor).
    pub vitals: VitalsParams,
}

impl Config {
    /// The frozen reference configuration for a layout.
    #[must_use]
    pub fn default_reference(layout: SubcarrierLayout) -> Self {
        Config {
            layout,
            band_breathing: Band::new(0.10, 0.60),
            band_heart: Band::new(0.80, 2.50),
            band_motion: Band::new(0.10, 5.00),
            uniform_fs_hz: 20.0,
            stft_window: 32,
            stft_hop: 8,
            select_k: 8,
            vitals: VitalsParams::default(),
        }
    }

    /// The resolved vitals-estimation configuration for this pipeline.
    #[must_use]
    pub fn vitals_config(&self) -> VitalsConfig {
        VitalsConfig {
            band_breathing: self.band_breathing,
            band_heart: self.band_heart,
            fs_hz: self.uniform_fs_hz,
            params: self.vitals,
        }
    }

    /// Validate the layout, bands, rate and STFT shape, and confirm every band
    /// yields a stable filter (`F1`, `L1`).
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.layout.validate()?;
        if !self.uniform_fs_hz.is_finite() || self.uniform_fs_hz <= 0.0 {
            return Err(ConfigError::BadRate);
        }
        if self.stft_window == 0 || self.stft_hop == 0 {
            return Err(ConfigError::BadStftShape);
        }
        for band in [self.band_breathing, self.band_heart, self.band_motion] {
            band.validate(self.uniform_fs_hz)?;
            // Build the section once so an unstable filter is rejected here.
            let _ = BiquadCoeffs::bandpass(band.lo_hz, band.hi_hz, self.uniform_fs_hz)?;
        }
        if !self.vitals.search_step_hz.is_finite()
            || self.vitals.search_step_hz <= 0.0
            || self.vitals.min_samples == 0
            || !self.vitals.min_snr_db.is_finite()
            || !self.vitals.confidence_scale_db.is_finite()
            || self.vitals.confidence_scale_db <= 0.0
            || self.vitals.stft_window == 0
            || self.vitals.stft_hop == 0
        {
            return Err(ConfigError::BadVitalsParams);
        }
        Ok(())
    }
}

/// Per-node conditioning output.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeFeatures {
    /// The node these features belong to.
    pub node: NodeId,
    /// The selected subcarrier indices, ascending.
    pub selected: Vec<u16>,
    /// Per-usable-subcarrier amplitude RMS, in usable order.
    pub amplitude_rms: Vec<f32>,
    /// Per-usable-subcarrier motion energy, in usable order.
    pub motion_energy: Vec<f32>,
    /// Per-usable-subcarrier noise-floor power, in usable order: the out-of-band
    /// residual power the motion bandpass left behind. This is the real noise
    /// term behind the SNR subcarrier selection (`signal.md` §4.11,
    /// `sister-issues-core.md` S6); it is never a hardcoded zero.
    pub noise_power: Vec<f32>,
    /// Breathing-rate candidates over time.
    pub breathing: Vec<BvpSample>,
    /// Heart-rate candidates over time.
    pub heart: Vec<BvpSample>,
    /// The deterministic breathing and heart rate estimates for the chunk.
    pub vitals: VitalsEstimates,
    /// Whether every usable subcarrier resampled onto the uniform grid.
    ///
    /// `false` means at least one subcarrier could not be resampled (a
    /// non-monotone timestamp, an over-large gap or a too-short span), so the
    /// series was carried through un-resampled. That is **unknown**, and is
    /// flagged rather than silently substituted (`sister-issues-core.md` S16).
    pub grid_is_uniform: bool,
}

impl NodeFeatures {
    /// Pack this node's conditioning output into the canonical
    /// [`FeatureVector`] that `aquila-inference` consumes
    /// (`data-contracts.md` §1, `signal.md` §2.4).
    ///
    /// Layout: every usable-subcarrier amplitude RMS value, then every
    /// usable-subcarrier motion energy, both in ascending usable-subcarrier
    /// order. The layout is owned here and is the single source the model
    /// seam's tensor projection reads.
    #[must_use]
    pub fn feature_vector(&self) -> FeatureVector {
        let mut values = Vec::with_capacity(self.amplitude_rms.len() + self.motion_energy.len());
        values.extend_from_slice(&self.amplitude_rms);
        values.extend_from_slice(&self.motion_energy);
        FeatureVector::new(values)
    }
}

/// The assembled features for a chunk.
#[derive(Debug, Clone, PartialEq)]
pub struct Features {
    /// Per-node features, in first-appearance order.
    pub nodes: Vec<NodeFeatures>,
    /// Sequence anomalies observed in the chunk.
    pub anomalies: Vec<SeqAnomaly>,
    /// The calibration state carried from the input.
    pub calibration: CalibrationState,
}

impl Features {
    /// A deterministic byte encoding of every field, for bit-exactness checks.
    ///
    /// The encoding is fixed (little-endian integers, `f32::to_bits`
    /// little-endian), so two equal encodings imply bit-identical `f32` fields,
    /// including signed zero. It is pure and performs no IO.
    #[must_use]
    pub fn fingerprint(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(match self.calibration {
            CalibrationState::Uncalibrated => 0,
            CalibrationState::Calibrated => 1,
            CalibrationState::Partial => 2,
        });
        push_len(&mut out, self.nodes.len());
        for node in &self.nodes {
            out.extend_from_slice(&node.node.0.to_le_bytes());
            push_len(&mut out, node.selected.len());
            for &s in &node.selected {
                out.extend_from_slice(&s.to_le_bytes());
            }
            push_f32s(&mut out, &node.amplitude_rms);
            push_f32s(&mut out, &node.motion_energy);
            push_bvp(&mut out, &node.breathing);
            push_bvp(&mut out, &node.heart);
            push_vital(&mut out, &node.vitals.breathing);
            push_vital(&mut out, &node.vitals.heart);
            out.push(u8::from(node.grid_is_uniform));
        }
        push_len(&mut out, self.anomalies.len());
        for anomaly in &self.anomalies {
            match anomaly {
                SeqAnomaly::Gap { from, to } => {
                    out.push(0);
                    out.extend_from_slice(&from.to_le_bytes());
                    out.extend_from_slice(&to.to_le_bytes());
                }
                SeqAnomaly::Reorder { expected, found } => {
                    out.push(1);
                    out.extend_from_slice(&expected.to_le_bytes());
                    out.extend_from_slice(&found.to_le_bytes());
                }
                SeqAnomaly::Regression { previous, current } => {
                    out.push(2);
                    out.extend_from_slice(&previous.to_le_bytes());
                    out.extend_from_slice(&current.to_le_bytes());
                }
                SeqAnomaly::Duplicate(seq) => {
                    out.push(3);
                    out.extend_from_slice(&seq.to_le_bytes());
                }
            }
        }
        out
    }

    /// Pack one node's features into the canonical [`FeatureVector`], or `None`
    /// when the chunk carries no features for that node.
    #[must_use]
    pub fn feature_vector(&self, node: NodeId) -> Option<FeatureVector> {
        self.nodes
            .iter()
            .find(|n| n.node == node)
            .map(NodeFeatures::feature_vector)
    }
}

fn push_len(out: &mut Vec<u8>, value: usize) {
    out.extend_from_slice(&(value as u64).to_le_bytes());
}

fn push_f32s(out: &mut Vec<u8>, values: &[f32]) {
    push_len(out, values.len());
    for &v in values {
        out.extend_from_slice(&v.to_bits().to_le_bytes());
    }
}

fn push_vital(out: &mut Vec<u8>, estimate: &VitalEstimate) {
    out.extend_from_slice(&estimate.rate_hz.to_bits().to_le_bytes());
    out.extend_from_slice(&estimate.rate_per_min.to_bits().to_le_bytes());
    out.extend_from_slice(&estimate.confidence.to_bits().to_le_bytes());
    out.extend_from_slice(&estimate.snr_db.to_bits().to_le_bytes());
    out.push(status_code(estimate.status));
}

fn status_code(status: VitalsStatus) -> u8 {
    match status {
        VitalsStatus::Ok => 0,
        VitalsStatus::InsufficientSignal => 1,
        VitalsStatus::MotionArtifact => 2,
        VitalsStatus::NoSubject => 3,
        VitalsStatus::CalibrationStale => 4,
        VitalsStatus::Unknown => 255,
    }
}

fn push_bvp(out: &mut Vec<u8>, samples: &[BvpSample]) {
    push_len(out, samples.len());
    for s in samples {
        out.extend_from_slice(&s.t.to_bits().to_le_bytes());
        out.extend_from_slice(&s.freq_hz.to_bits().to_le_bytes());
        out.extend_from_slice(&s.snr_db.to_bits().to_le_bytes());
    }
}

/// Run the reference pipeline over one chunk (`signal.md` §5).
///
/// The chunk is already-decoded CSI frames (decode happens at the transport
/// seam). Frames are grouped by node in first-appearance order; per node the
/// pipeline normalises, projects the usable bins, unwraps and sanitises phase,
/// resamples onto the uniform grid, band-limits into the motion band, extracts
/// motion energies, selects subcarriers and tracks breathing/heart candidates.
///
/// # Errors
///
/// [`SignalError`] for an empty chunk, an invalid config, a calibration shape
/// mismatch, or any non-finite bin.
pub fn process(
    chunk: &[ComplexFrame],
    cal: &Calibration,
    cfg: &Config,
) -> Result<Features, SignalError> {
    if chunk.is_empty() {
        return Err(SignalError::EmptyChunk);
    }
    cfg.validate()?;
    if !cal.per_node.is_empty() && cal.per_node.len() != cfg.layout.expected_bins {
        return Err(SignalError::CalibrationShape {
            actual: cal.per_node.len(),
            expected: cfg.layout.expected_bins,
        });
    }
    for frame in chunk {
        for (i, bin) in frame.bins.iter().enumerate() {
            if !bin.is_finite() {
                return Err(SignalError::NonFiniteSample(i));
            }
        }
    }

    let motion_coeffs = BiquadCoeffs::bandpass(
        cfg.band_motion.lo_hz,
        cfg.band_motion.hi_hz,
        cfg.uniform_fs_hz,
    )?;
    let breathing_coeffs = BiquadCoeffs::bandpass(
        cfg.band_breathing.lo_hz,
        cfg.band_breathing.hi_hz,
        cfg.uniform_fs_hz,
    )?;
    let heart_coeffs = BiquadCoeffs::bandpass(
        cfg.band_heart.lo_hz,
        cfg.band_heart.hi_hz,
        cfg.uniform_fs_hz,
    )?;
    let stft_cfg = StftConfig {
        window: cfg.stft_window,
        hop: cfg.stft_hop,
        fs_hz: cfg.uniform_fs_hz,
    };
    let vitals_cfg = cfg.vitals_config();

    // Group by node, preserving first-appearance order.
    let mut groups: Vec<(NodeId, Vec<&ComplexFrame>)> = Vec::new();
    for frame in chunk {
        if let Some(group) = groups.iter_mut().find(|(node, _)| *node == frame.meta.node) {
            group.1.push(frame);
        } else {
            groups.push((frame.meta.node, vec![frame]));
        }
    }

    let mut anomalies = Vec::new();
    let mut nodes = Vec::with_capacity(groups.len());
    for (node, frames) in &groups {
        let seqs: Vec<u32> = frames.iter().map(|f| f.meta.seq).collect();
        anomalies.extend(anomalies_for(&seqs));

        let normalised: Vec<_> = frames.iter().map(|f| normalise(f, cal)).collect();
        let usable = &cfg.layout.usable;
        let n_use = usable.len();
        let n_frames = normalised.len();
        let t_us: Vec<u64> = frames.iter().map(|f| f.meta.t_device_us).collect();

        // Per-usable-bin amplitude and sanitised-phase series over time.
        let mut amp_series = vec![vec![0.0_f32; n_frames]; n_use];
        let mut phase_series = vec![vec![0.0_f32; n_frames]; n_use];
        for (m, frame) in normalised.iter().enumerate() {
            let projected = cfg.layout.project(&frame.bins);
            let mut wrapped = Vec::with_capacity(projected.len());
            for (b, bin) in projected.iter().enumerate() {
                amp_series[b][m] = bin.magnitude();
                wrapped.push(atan2_approx(bin.im, bin.re));
            }
            let unwrapped = unwrap_phase(&wrapped);
            let sanitised = sanitise_phase(&unwrapped, &cfg.layout);
            for (b, value) in sanitised.residual.iter().enumerate().take(n_use) {
                phase_series[b][m] = *value;
            }
        }

        let mut amplitude_rms = Vec::with_capacity(n_use);
        let mut motion_energy = Vec::with_capacity(n_use);
        let mut noise_power = Vec::with_capacity(n_use);
        let mut motion_series: Vec<Vec<f32>> = Vec::with_capacity(n_use);
        // A resample failure is flagged, not hidden: the raw series is carried
        // through but `grid_is_uniform` records that the grid is unknown
        // (`sister-issues-core.md` S16).
        let mut grid_is_uniform = true;
        for b in 0..n_use {
            let amp_grid = match resample_uniform(&amp_series[b], &t_us, cfg.uniform_fs_hz) {
                Ok(grid) => grid,
                Err(_) => {
                    grid_is_uniform = false;
                    amp_series[b].clone()
                }
            };
            let phase_grid = match resample_uniform(&phase_series[b], &t_us, cfg.uniform_fs_hz) {
                Ok(grid) => grid,
                Err(_) => {
                    grid_is_uniform = false;
                    phase_series[b].clone()
                }
            };
            amplitude_rms.push(rms(&amp_grid));
            let motion = bandpass(&phase_grid, &motion_coeffs);
            let energy: f32 = motion_features(&motion, cfg.stft_window).into_iter().sum();
            motion_energy.push(energy);
            // A real per-subcarrier noise floor: the out-of-band residual power
            // of the phase series, i.e. what the motion bandpass left behind
            // (`sister-issues-core.md` S6). The old code passed a hardcoded zero,
            // so the documented SNR score degenerated to raw band power.
            noise_power.push(residual_noise_power(&phase_grid, &motion));
            motion_series.push(motion);
        }

        let matrix = SubcarrierFeatureMatrix::new(
            usable.clone(),
            motion_energy.clone(),
            noise_power.clone(),
        );
        let selected = select_subcarriers(&matrix, cfg.select_k).selected;

        // Mean motion series across the selected bins, band-limited per band.
        let (breathing, heart, vitals) = if selected.is_empty() {
            (Vec::new(), Vec::new(), estimate_vitals(&[], &vitals_cfg))
        } else {
            let grid_len = motion_series.first().map(|s| s.len()).unwrap_or_default();
            let mut mean_series = vec![0.0_f32; grid_len];
            for &index in &selected {
                if let Some(pos) = usable.iter().position(|&u| u == index) {
                    for (acc, value) in mean_series.iter_mut().zip(&motion_series[pos]) {
                        *acc += *value;
                    }
                }
            }
            let scale = 1.0 / selected.len() as f32;
            for value in &mut mean_series {
                *value *= scale;
            }
            let breath_series = bandpass(&mean_series, &breathing_coeffs);
            let heart_series = bandpass(&mean_series, &heart_coeffs);
            let breath_spec = stft(&breath_series, &stft_cfg);
            let heart_spec = stft(&heart_series, &stft_cfg);
            (
                bvp_track(&breath_spec, cfg.band_breathing),
                bvp_track(&heart_spec, cfg.band_heart),
                estimate_vitals(&mean_series, &vitals_cfg),
            )
        };

        nodes.push(NodeFeatures {
            node: *node,
            selected,
            amplitude_rms,
            motion_energy,
            noise_power,
            breathing,
            heart,
            vitals,
            grid_is_uniform,
        });
    }

    Ok(Features {
        nodes,
        anomalies,
        calibration: cal.state,
    })
}

/// The out-of-band residual power of a phase series, a real per-subcarrier
/// noise-floor estimate (`signal.md` §4.11).
///
/// `residual = signal − filtered` is what the motion bandpass left behind; its
/// variance is the power the signal band does not explain, so `band / noise` in
/// [`select_subcarriers`] is a genuine SNR, not a raw-power fallback.
#[must_use]
fn residual_noise_power(signal: &[f32], filtered: &[f32]) -> f32 {
    let n = signal.len().min(filtered.len());
    if n == 0 {
        return 0.0;
    }
    let mut mean = 0.0_f64;
    let mut sumsq = 0.0_f64;
    for i in 0..n {
        let residual = f64::from(signal[i] - filtered[i]);
        mean += residual;
        sumsq += residual * residual;
    }
    let mean = mean / n as f64;
    let variance = (sumsq / n as f64) - mean * mean;
    if variance.is_finite() && variance > 0.0 {
        variance as f32
    } else {
        0.0
    }
}
