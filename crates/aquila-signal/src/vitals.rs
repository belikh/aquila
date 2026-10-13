//! Deterministic vital-rate estimation.
//!
//! This module turns a conditioned, uniformly-sampled series (a BVP-style
//! motion series — the residual-phase motion the pipeline already extracts) into
//! a breathing-rate and heart-rate estimate, each with a confidence and a
//! status. It reuses the existing conditioning stages: [`bandpass`] isolates the
//! physiological band, [`stft`]/[`bvp_track`] give a coarse dominant-bin
//! candidate, and [`resonator`] (a sliding Goertzel) refines the peak to
//! sub-bin precision.
//!
//! # Why a resonator scan
//!
//! A short-time Fourier bin is `fs / W` wide — at `20 Hz` and a 256-sample
//! window that is `0.078 Hz` (`≈ 4.7 brpm`), coarser than the `≤ 2 brpm` goal.
//! The *location* of a single tone's spectral peak, however, can be found to
//! arbitrary precision by evaluating the continuous frequency response and
//! refining the three samples around the maximum with a fixed parabolic
//! formula (`signal.md` §4.10). The [`resonator`] stage provides that
//! frequency response, so the estimator reaches sub-`0.01 Hz` accuracy on a
//! clean tone without a platform transcendental: the Goertzel twiddles use the
//! crate's pinned `sin`/`cos` approximations.
//!
//! # Honesty
//!
//! The estimator never fabricates a rate. Below its in-band SNR floor it
//! returns [`VitalsStatus::InsufficientSignal`] with a zero rate and zero
//! confidence; the caller must not surface a value for a non-`Ok` status
//! (`data-contracts.md` §5, invariant 3).

use aquila_types::VitalsStatus;

use crate::filter::{bandpass, resonator, Band, BiquadCoeffs};
use crate::math::log10_approx;
use crate::spectrogram::{bvp_track, stft, StftConfig};

/// The vitals-specific knobs carried by the reference [`Config`](crate::Config).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VitalsParams {
    /// The frequency step of the resonator scan, in hertz. Smaller is more
    /// precise and more expensive; `0.002 Hz` is `0.12 brpm`.
    pub search_step_hz: f32,
    /// The minimum series length, in samples, before estimation is attempted.
    pub min_samples: usize,
    /// The in-band SNR floor, in decibels, below which no rate is claimed.
    pub min_snr_db: f32,
    /// The SNR range, in decibels, above the floor over which the reported
    /// confidence rises from `0` to `1`.
    pub confidence_scale_db: f32,
    /// The STFT window, in samples, used by the coarse BVP candidate. It must
    /// be long enough to place a bin inside the respiratory band: at `20 Hz` a
    /// 32-sample window is `0.625 Hz` wide and leaves the `0.10..0.50 Hz` band
    /// with no bin, so the vitals window is deliberately longer than the
    /// presence STFT window.
    pub stft_window: usize,
    /// The STFT hop, in samples, for the coarse BVP candidate.
    pub stft_hop: usize,
}

impl Default for VitalsParams {
    fn default() -> Self {
        Self {
            search_step_hz: 0.002,
            min_samples: 64,
            min_snr_db: 6.0,
            confidence_scale_db: 12.0,
            stft_window: 256,
            stft_hop: 64,
        }
    }
}

/// The fully-resolved configuration of one vitals estimation: the physiological
/// bands, the sampling rate and the search parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VitalsConfig {
    /// The respiratory band, in hertz (default `0.10..0.50`).
    pub band_breathing: Band,
    /// The cardiac band, in hertz (default `0.80..2.00`).
    pub band_heart: Band,
    /// The uniform sampling rate of the series, in hertz.
    pub fs_hz: f32,
    /// The vitals search parameters, including the coarse-candidate STFT shape.
    pub params: VitalsParams,
}

/// One rate estimate with its confidence and status.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VitalEstimate {
    /// The estimated rate, in hertz, or `0.0` when the status is not `Ok`.
    pub rate_hz: f32,
    /// The estimated rate, in the field's unit (brpm or bpm), or `0.0` when the
    /// status is not `Ok`.
    pub rate_per_min: f32,
    /// The model's confidence in `[0, 1]`, or `0.0` when the status is not
    /// `Ok`.
    pub confidence: f32,
    /// The in-band peak-to-mean power ratio of the estimate, in decibels.
    pub snr_db: f32,
    /// The status of the estimate. Anything but `Ok` means no rate is claimed.
    pub status: VitalsStatus,
}

impl VitalEstimate {
    /// The insufficient-signal outcome: no rate is claimed.
    #[must_use]
    fn insufficient(snr_db: f32) -> Self {
        Self {
            rate_hz: 0.0,
            rate_per_min: 0.0,
            confidence: 0.0,
            snr_db,
            status: VitalsStatus::InsufficientSignal,
        }
    }

    /// Whether a rate is claimed.
    #[must_use]
    pub const fn is_ok(&self) -> bool {
        matches!(self.status, VitalsStatus::Ok)
    }
}

/// The breathing and heart estimates for one conditioned series.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VitalsEstimates {
    /// The breathing-rate estimate, in breaths per minute.
    pub breathing: VitalEstimate,
    /// The heart-rate estimate, in beats per minute. Experimental in v1.
    pub heart: VitalEstimate,
}

/// Estimate the breathing and heart rates of one conditioned series.
///
/// The series is the residual-phase motion (the BVP-style signal) sampled
/// uniformly at [`VitalsConfig::fs_hz`]. The function is pure and deterministic:
/// identical input and configuration yield bit-identical estimates.
#[must_use]
pub fn estimate_vitals(series: &[f32], config: &VitalsConfig) -> VitalsEstimates {
    VitalsEstimates {
        breathing: estimate_band(series, config.band_breathing, config),
        heart: estimate_band(series, config.band_heart, config),
    }
}

/// Estimate the dominant rate of `series` within one band.
///
/// Bands the series, takes a coarse dominant-bin candidate from the
/// spectrogram/BVP stage, then refines the peak with a resonator scan and a
/// parabolic fit. Returns [`VitalsStatus::InsufficientSignal`] — never a
/// fabricated rate — when the series is too short, the band is invalid, or the
/// in-band SNR is below the configured floor.
#[must_use]
pub fn estimate_band(series: &[f32], band: Band, config: &VitalsConfig) -> VitalEstimate {
    let params = &config.params;
    if series.len() < params.min_samples
        || !config.fs_hz.is_finite()
        || config.fs_hz <= 0.0
        || !params.search_step_hz.is_finite()
        || params.search_step_hz <= 0.0
        || !band.lo_hz.is_finite()
        || !band.hi_hz.is_finite()
        || band.lo_hz >= band.hi_hz
    {
        return VitalEstimate::insufficient(0.0);
    }

    let Ok(coeffs) = BiquadCoeffs::bandpass(band.lo_hz, band.hi_hz, config.fs_hz) else {
        return VitalEstimate::insufficient(0.0);
    };
    let filtered = bandpass(series, &coeffs);

    // Coarse candidate from the existing spectrogram/BVP stages. An empty track
    // means the filtered series carries nothing to track.
    let stft_cfg = StftConfig {
        window: params.stft_window,
        hop: params.stft_hop,
        fs_hz: config.fs_hz,
    };
    let track = bvp_track(&stft(&filtered, &stft_cfg), band);
    if track.is_empty() {
        return VitalEstimate::insufficient(0.0);
    }

    // Resonator scan over the band for the refined peak.
    let lo = band.lo_hz.max(0.0);
    let hi = band.hi_hz.min(config.fs_hz / 2.0);
    if hi <= lo {
        return VitalEstimate::insufficient(0.0);
    }
    let step = params.search_step_hz;
    let count = ((hi - lo) / step).floor() as usize + 1;
    if count == 0 {
        return VitalEstimate::insufficient(0.0);
    }
    let mut powers = Vec::with_capacity(count);
    let mut best_index = 0usize;
    let mut best_power = f32::MIN;
    let mut total = 0.0_f32;
    for i in 0..count {
        let f = lo + step * i as f32;
        let power = resonator(&filtered, f, config.fs_hz).magnitude_sq();
        powers.push(power);
        total += power;
        if power > best_power {
            best_power = power;
            best_index = i;
        }
    }
    let mean_power = total / count as f32;
    let snr_db = if mean_power > 0.0 && best_power > 0.0 {
        10.0 * log10_approx(best_power / mean_power)
    } else {
        0.0
    };

    // Parabolic refinement of the peak using its two neighbours.
    let mut peak_hz = lo + step * best_index as f32;
    if best_index > 0 && best_index + 1 < count {
        let y_prev = powers[best_index - 1];
        let y_peak = powers[best_index];
        let y_next = powers[best_index + 1];
        let denom = y_prev - 2.0 * y_peak + y_next;
        if denom.is_finite() && denom != 0.0 {
            let delta = 0.5 * (y_prev - y_next) / denom;
            if delta.is_finite() && delta.abs() <= 1.0 {
                peak_hz += delta * step;
            }
        }
    }

    if snr_db >= params.min_snr_db && peak_hz.is_finite() {
        let confidence =
            ((snr_db - params.min_snr_db) / params.confidence_scale_db).clamp(0.0, 1.0);
        VitalEstimate {
            rate_hz: peak_hz,
            rate_per_min: peak_hz * 60.0,
            confidence,
            snr_db,
            status: VitalsStatus::Ok,
        }
    } else {
        VitalEstimate::insufficient(snr_db)
    }
}
