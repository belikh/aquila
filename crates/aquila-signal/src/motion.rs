//! Motion / residual features.
//!
//! `motion_features` extracts clutter-removed motion energy from a series
//! (`signal.md` §4.9). The clutter estimate per window is the window mean; the
//! residual is the mean-removed window; the feature is the window energy,
//! accumulated in a stable two-pass form.

/// Per-window motion energy over non-overlapping windows of length `window`.
///
/// For each window, `μ = Σx_j / W` then `E = Σ(x_j − μ)²`, accumulated in
/// `f64` and narrowed once. Translation-invariant (`M1`), `c²`-homogeneous
/// (`M2`), non-negative and zero iff the window is constant (`M3`), and
/// deterministic with a fixed order (`M4`).
///
/// A zero window returns an empty vector. A trailing partial window is
/// evaluated over the samples it has and is not padded (`M5`); the caller can
/// detect it from the input length.
#[must_use]
pub fn motion_features(x: &[f32], window: usize) -> Vec<f32> {
    if window == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(x.len().div_ceil(window));
    for chunk in x.chunks(window) {
        let n = chunk.len();
        if n == 0 {
            continue;
        }
        let mut sum = 0.0_f64;
        for &v in chunk {
            sum += f64::from(v);
        }
        let mean = sum / (n as f64);
        let mut energy = 0.0_f64;
        for &v in chunk {
            let d = f64::from(v) - mean;
            energy += d * d;
        }
        out.push(energy as f32);
    }
    out
}

/// The root-mean-square of a series, accumulated in `f64`.
///
/// Returns `0.0` for an empty series. Used by the pipeline to summarise an
/// amplitude series per subcarrier.
#[must_use]
pub fn rms(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    let mut sum = 0.0_f64;
    for &v in x {
        sum += f64::from(v) * f64::from(v);
    }
    (sum / x.len() as f64).sqrt() as f32
}
