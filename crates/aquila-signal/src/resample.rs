//! Uniform resampling precondition.
//!
//! CSI arrivals are event-driven and jittered; the STFT needs a uniform grid.
//! `resample_uniform` maps device timestamps to a uniform grid by deterministic
//! linear interpolation (`signal.md` §4.8`). It does not estimate drift; the
//! device→common time base map is the caller's responsibility.

use crate::error::ResampleError;

/// Resample `x` onto a uniform grid at `fs_hz`, using linear interpolation.
///
/// The grid origin is the first timestamp and the spacing is `1e6 / fs_hz`
/// microseconds (`R1`). An already-uniform grid is reproduced exactly (`R2`).
/// Timestamps must be non-decreasing (`R4`); a gap exceeding four times the
/// median spacing is reported as [`ResampleError::TooIrregular`] rather than
/// inventing samples (`R3`).
///
/// # Errors
///
/// [`ResampleError`] for a length mismatch, fewer than two samples, a bad rate,
/// non-monotone times, an over-large gap, or a zero-length span.
pub fn resample_uniform(x: &[f32], t_us: &[u64], fs_hz: f32) -> Result<Vec<f32>, ResampleError> {
    if x.len() != t_us.len() {
        return Err(ResampleError::LengthMismatch {
            series: x.len(),
            times: t_us.len(),
        });
    }
    if x.len() < 2 {
        return Err(ResampleError::TooFewSamples(x.len()));
    }
    if !fs_hz.is_finite() || fs_hz <= 0.0 {
        return Err(ResampleError::BadRate);
    }
    for i in 1..t_us.len() {
        if t_us[i] < t_us[i - 1] {
            return Err(ResampleError::NonMonotone(i));
        }
    }

    let mut diffs: Vec<u64> = Vec::with_capacity(t_us.len() - 1);
    for i in 1..t_us.len() {
        diffs.push(t_us[i] - t_us[i - 1]);
    }
    let mut sorted = diffs.clone();
    sorted.sort_unstable();
    let median = sorted[sorted.len() / 2].max(1);
    for (i, &gap) in diffs.iter().enumerate() {
        if gap > median.saturating_mul(4) {
            return Err(ResampleError::TooIrregular {
                index: i + 1,
                gap_us: gap,
            });
        }
    }

    let origin = t_us[0];
    let span = t_us[t_us.len() - 1] - origin;
    if span == 0 {
        return Err(ResampleError::SpanTooShort);
    }
    let interval = 1_000_000.0_f64 / f64::from(fs_hz);
    let count = (span as f64 / interval).floor() as usize + 1;

    let mut out = Vec::with_capacity(count);
    let mut j = 0usize;
    for k in 0..count {
        let target = origin as f64 + (k as f64) * interval;
        while j + 1 < t_us.len() && (t_us[j + 1] as f64) < target {
            j += 1;
        }
        if j + 1 >= t_us.len() {
            out.push(x[x.len() - 1]);
            continue;
        }
        let low = t_us[j] as f64;
        let high = t_us[j + 1] as f64;
        if high <= low {
            out.push(x[j]);
            continue;
        }
        let w = (target - low) / (high - low);
        let value = f64::from(x[j]) * (1.0 - w) + f64::from(x[j + 1]) * w;
        out.push(value as f32);
    }
    Ok(out)
}
