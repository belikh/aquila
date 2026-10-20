//! Spectrogram and BVP extraction.
//!
//! `stft` computes a short-time Fourier transform using a `const`-shaped Hann
//! window and deterministic twiddles; `bvp_track` band-limits `|X|` to a band
//! and picks the dominant bin per frame (`signal.md` §4.10). No platform
//! `sin`/`cos`/`log` runs: the approximations in [`crate::math`] are pinned.

use crate::decode::Complex32;
use crate::filter::Band;
use crate::math::{cos_approx, log10_approx, sin_approx, TAU};

/// The STFT configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StftConfig {
    /// The window length, in samples.
    pub window: usize,
    /// The hop between successive frames, in samples.
    pub hop: usize,
    /// The sampling rate of the input series, in hertz.
    pub fs_hz: f32,
}

/// A full-bin short-time Fourier transform.
///
/// `values` is row-major, frame-major: frame `m`, bin `k` is at
/// `values[m * bin_count + k]`. Bin `k` maps to `f_k = k · fs / W`; bins above
/// `W/2` are the negative-frequency half (`B2`).
#[derive(Debug, Clone, PartialEq)]
pub struct Spectrogram {
    /// The number of frequency bins per frame (the window length).
    pub bin_count: usize,
    /// The number of time frames.
    pub frame_count: usize,
    /// The sampling rate, carried for the frequency mapping.
    pub fs_hz: f32,
    /// The hop, in samples, carried for the time stamp of each frame.
    pub hop: usize,
    /// Row-major complex values, `frame_count × bin_count`.
    pub values: Vec<Complex32>,
}

impl Spectrogram {
    /// The value at `(frame, bin)`, or [`Complex32::ZERO`] if out of range.
    #[must_use]
    pub fn value(&self, frame: usize, bin: usize) -> Complex32 {
        if frame >= self.frame_count || bin >= self.bin_count {
            return Complex32::ZERO;
        }
        self.values[frame * self.bin_count + bin]
    }
}

/// One breathing/heart-rate candidate at one time frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BvpSample {
    /// The time of the frame, in seconds from the start of the series.
    pub t: f32,
    /// The dominant frequency within the band, in hertz.
    pub freq_hz: f32,
    /// The peak-to-mean power ratio within the band, in decibels.
    pub snr_db: f32,
}

/// Compute the full-bin STFT of `x`.
///
/// `X[m,k] = Σ_{n<W} w[n]·x[mH+n]·e^{−i2πkn/W}` with a Hann window. A window
/// longer than the signal zero-pads deterministically (`B6`). `W == 0` yields
/// an empty spectrogram. The transform is linear and deterministic (`B1`,
/// `B3`).
#[must_use]
pub fn stft(x: &[f32], cfg: &StftConfig) -> Spectrogram {
    let w = cfg.window;
    if w == 0 {
        return Spectrogram {
            bin_count: 0,
            frame_count: 0,
            fs_hz: cfg.fs_hz,
            hop: cfg.hop,
            values: Vec::new(),
        };
    }
    let hop = cfg.hop.max(1);
    let n = x.len();
    let frames = if n >= w { (n - w) / hop + 1 } else { 1 };

    // Hann window: 0.5 − 0.5·cos(2πn/(W−1)); a single-sample window is 1.
    let window: Vec<f32> = (0..w)
        .map(|i| {
            if w == 1 {
                1.0
            } else {
                let angle = TAU * (i as f32) / ((w - 1) as f32);
                0.5 - 0.5 * cos_approx(angle)
            }
        })
        .collect();

    let mut values = vec![Complex32::ZERO; frames * w];
    for m in 0..frames {
        let start = m * hop;
        for k in 0..w {
            let angle_step = -TAU * (k as f32) / (w as f32);
            let mut real = 0.0_f32;
            let mut imag = 0.0_f32;
            for (i, &window_value) in window.iter().enumerate() {
                let Some(&sample) = x.get(start + i) else {
                    break;
                };
                let angle = angle_step * (i as f32);
                let weighted = window_value * sample;
                real += weighted * cos_approx(angle);
                imag += weighted * sin_approx(angle);
            }
            values[m * w + k] = Complex32::new(real, imag);
        }
    }

    Spectrogram {
        bin_count: w,
        frame_count: frames,
        fs_hz: cfg.fs_hz,
        hop,
        values,
    }
}

/// Track the dominant in-band frequency over the frames of a spectrogram.
///
/// Bins outside `[0, fs/2]` are ignored; an invalid or empty band yields an
/// empty track. Ties resolve to the lowest bin (`B5` determinism). `snr_db` is
/// the peak-to-mean power ratio within the band, or `0.0` when the band is
/// silent.
#[must_use]
pub fn bvp_track(spec: &Spectrogram, band: Band) -> Vec<BvpSample> {
    if spec.bin_count == 0 || spec.frame_count == 0 || spec.fs_hz <= 0.0 {
        return Vec::new();
    }
    if !band.lo_hz.is_finite() || !band.hi_hz.is_finite() || band.lo_hz >= band.hi_hz {
        return Vec::new();
    }
    let nyquist_bin = spec.bin_count / 2;
    let mut low = (band.lo_hz * spec.bin_count as f32 / spec.fs_hz).ceil() as isize;
    let mut high = (band.hi_hz * spec.bin_count as f32 / spec.fs_hz).floor() as isize;
    low = low.max(0);
    high = high.min(nyquist_bin as isize);
    if high < low {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(spec.frame_count);
    for m in 0..spec.frame_count {
        let mut best_bin = low as usize;
        let mut best_power = -1.0_f32;
        let mut total = 0.0_f32;
        let mut count = 0usize;
        for k in (low as usize)..=(high as usize) {
            let power = spec.value(m, k).magnitude_sq();
            total += power;
            count += 1;
            if power > best_power {
                best_power = power;
                best_bin = k;
            }
        }
        let mean = if count > 0 { total / count as f32 } else { 0.0 };
        let snr_db = if mean > 0.0 && best_power > 0.0 {
            10.0 * log10_approx(best_power / mean)
        } else {
            0.0
        };
        let freq = best_bin as f32 * spec.fs_hz / spec.bin_count as f32;
        let t = (m as f32) * (spec.hop as f32) / spec.fs_hz;
        out.push(BvpSample {
            t,
            freq_hz: freq,
            snr_db,
        });
    }
    out
}
