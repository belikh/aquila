//! Bandpass and resonator filtering.
//!
//! `bandpass` is a cascade of second-order sections (RBJ-style bandpass) with
//! zero initial state; `resonator` is a sliding Goertzel returning the complex
//! band value at one frequency (`signal.md` §4.7). Coefficients are built from
//! a band once (offline or at config time) using the deterministic
//! approximations in [`crate::math`], never platform trig.

use crate::decode::Complex32;
use crate::error::ConfigError;
use crate::math::{cos_approx, sin_approx, TAU};

/// A frequency band, in hertz.
///
/// This is the signal-frequency band used by the conditioning pipeline. It is
/// deliberately distinct from the radio band
/// [`aquila_types::Band`](aquila_types::Band).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    /// The lower band edge, in hertz.
    pub lo_hz: f32,
    /// The upper band edge, in hertz.
    pub hi_hz: f32,
}

impl Band {
    /// Construct a band.
    #[must_use]
    pub const fn new(lo_hz: f32, hi_hz: f32) -> Self {
        Band { lo_hz, hi_hz }
    }

    /// Validate the band against a sampling rate: finite, `0 ≤ lo < hi ≤ fs/2`.
    pub fn validate(&self, fs_hz: f32) -> Result<(), ConfigError> {
        let nyquist = fs_hz / 2.0;
        let valid = self.lo_hz.is_finite()
            && self.hi_hz.is_finite()
            && fs_hz.is_finite()
            && fs_hz > 0.0
            && self.lo_hz >= 0.0
            && self.lo_hz < self.hi_hz
            && self.hi_hz <= nyquist;
        if valid {
            Ok(())
        } else {
            Err(ConfigError::BadBand {
                lo: self.lo_hz,
                hi: self.hi_hz,
                fs_hz,
            })
        }
    }
}

/// One normalised second-order section,
/// `y[n] = b0·x[n] + b1·x[n−1] + b2·x[n−2] − a1·y[n−1] − a2·y[n−2]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Biquad {
    /// Feed-forward coefficient for the current sample.
    pub b0: f32,
    /// Feed-forward coefficient for the previous sample.
    pub b1: f32,
    /// Feed-forward coefficient for the sample before that.
    pub b2: f32,
    /// Feedback coefficient for the previous output.
    pub a1: f32,
    /// Feedback coefficient for the output before that.
    pub a2: f32,
}

/// A cascade of biquad sections.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BiquadCoeffs {
    /// The sections, applied in order.
    pub sections: Vec<Biquad>,
}

impl BiquadCoeffs {
    /// Build a single-section RBJ constant-0-dB-peak-gain bandpass.
    ///
    /// `centre = √(lo·hi)`, `Q = centre / (hi − lo)`. The bilinear-transform
    /// tangent uses the deterministic [`tan_approx`]; no platform trig runs.
    ///
    /// # Errors
    ///
    /// [`ConfigError::BadBand`] for a degenerate or out-of-range band;
    /// [`ConfigError::UnstableFilter`] if the resulting poles fall outside the
    /// unit circle (`F1`).
    pub fn bandpass(lo_hz: f32, hi_hz: f32, fs_hz: f32) -> Result<Self, ConfigError> {
        Band::new(lo_hz, hi_hz).validate(fs_hz)?;
        let centre = (lo_hz * hi_hz).sqrt();
        let width = hi_hz - lo_hz;
        if width <= 0.0 {
            return Err(ConfigError::BadBand {
                lo: lo_hz,
                hi: hi_hz,
                fs_hz,
            });
        }
        let q = centre / width;
        let w0 = TAU * centre / fs_hz;
        let cos_w0 = cos_approx(w0);
        let alpha = sin_approx(w0) / (2.0 * q);
        let a0 = 1.0 + alpha;
        if !a0.is_finite() || a0 == 0.0 {
            return Err(ConfigError::NonFiniteFilter);
        }
        let section = Biquad {
            b0: alpha / a0,
            b1: 0.0,
            b2: -alpha / a0,
            a1: (-2.0 * cos_w0) / a0,
            a2: (1.0 - alpha) / a0,
        };
        if !section.b0.is_finite()
            || !section.b1.is_finite()
            || !section.b2.is_finite()
            || !section.a1.is_finite()
            || !section.a2.is_finite()
        {
            return Err(ConfigError::NonFiniteFilter);
        }
        if !section.is_stable() {
            return Err(ConfigError::UnstableFilter);
        }
        Ok(BiquadCoeffs {
            sections: vec![section],
        })
    }
}

impl Biquad {
    /// Check `F1`: both poles lie strictly inside the unit circle (Jury test).
    #[must_use]
    pub fn is_stable(&self) -> bool {
        self.a2.abs() < 1.0 && self.a1.abs() < 1.0 + self.a2
    }
}

/// Apply a section cascade to a series with zero initial state.
///
/// A fixed direct-form-I operation order (`F5`). Linearity (`F2`), zero-in /
/// zero-out (`F4`) and DC rejection (`F3`) follow. An empty cascade is the
/// identity.
#[must_use]
pub fn bandpass(x: &[f32], coeffs: &BiquadCoeffs) -> Vec<f32> {
    let mut buffer = x.to_vec();
    for section in &coeffs.sections {
        let (b0, b1, b2, a1, a2) = (
            section.b0,
            section.b1,
            section.b2,
            section.a1,
            section.a2,
        );
        let mut x1 = 0.0_f32;
        let mut x2 = 0.0_f32;
        let mut y1 = 0.0_f32;
        let mut y2 = 0.0_f32;
        for sample in buffer.iter_mut() {
            let xn = *sample;
            let yn = b0 * xn + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            x2 = x1;
            x1 = xn;
            y2 = y1;
            y1 = yn;
            *sample = yn;
        }
    }
    buffer
}

/// Sliding Goertzel: the complex band value of `x` at `at_hz`.
///
/// Uses the deterministic `sin`/`cos` approximations for the twiddle. A
/// non-positive rate, a non-positive frequency or an empty series yields
/// [`Complex32::ZERO`]. `F3`: a constant input at `at_hz > 0` gives a value
/// near zero.
#[must_use]
pub fn resonator(x: &[f32], at_hz: f32, fs_hz: f32) -> Complex32 {
    if x.is_empty() || fs_hz <= 0.0 || at_hz <= 0.0 || !fs_hz.is_finite() || !at_hz.is_finite() {
        return Complex32::ZERO;
    }
    let w0 = TAU * at_hz / fs_hz;
    let coeff = 2.0 * cos_approx(w0);
    let mut s1 = 0.0_f32;
    let mut s2 = 0.0_f32;
    for &sample in x {
        let s0 = sample + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    let real = s1 - s2 * cos_approx(w0);
    let imag = s2 * sin_approx(w0);
    Complex32::new(real, imag)
}
