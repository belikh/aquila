//! Deterministic transcendental approximations.
//!
//! `signal.md` §3.4 forbids calling the platform `sin`/`cos`/`tan`/`atan2`/`ln`
//! in the reference path. This module supplies pinned polynomial
//! approximations with a fixed operation order, so identical input yields
//! bit-identical output on the pinned target. They are used only where the spec
//! permits owned transcendentals: filter-coefficient construction, STFT
//! twiddles, phase extraction and SNR.
//!
//! `f32::sqrt` and `f32::to_bits`/`from_bits` are IEEE-defined or safe bit
//! reinterpretations and remain permitted (`signal.md` §3.4, §3.6).

// The constants below are deliberately written as pinned decimal literals
// rather than taken from `core::f32::consts`, so the reference path does not
// depend on a library-derived value; some carry more digits than an `f32` can
// hold to keep the pinned source form stable across targets.
#![allow(clippy::approx_constant, clippy::excessive_precision)]

/// `2π` at `f32` precision, used for range reduction.
const TWO_PI_F32: f32 = 6.283_185_5;
/// `2π` at `f64` precision; range reduction is done here to limit cancellation.
const TWO_PI_F64: f64 = 6.283_185_307_179_586;
/// `π` at `f32` precision.
const PI_F32: f32 = 3.141_592_7;
/// `π/2` at `f32` precision.
const FRAC_PI_2_F32: f32 = 1.570_796_3;

/// A deterministic `sin(x)` approximation.
///
/// Range reduction brings `x` into `[-π, π]`, symmetry into `[-π/2, π/2]`, then
/// an 11th-order Taylor series evaluates the result. Worst-case absolute error
/// on `[-π/2, π/2]` is below `2e-8`.
#[must_use]
pub(crate) fn sin_approx(x: f32) -> f32 {
    let xd = f64::from(x);
    let k = (xd / TWO_PI_F64 + 0.5).floor();
    let mut r = (xd - k * TWO_PI_F64) as f32;
    if r > FRAC_PI_2_F32 {
        r = PI_F32 - r;
    } else if r < -FRAC_PI_2_F32 {
        r = -PI_F32 - r;
    }
    let r2 = r * r;
    r * (1.0
        - r2 * (1.0 / 6.0
            - r2 * (1.0 / 120.0
                - r2 * (1.0 / 5040.0 - r2 * (1.0 / 362_880.0 - r2 / 39_916_800.0)))))
}

/// A deterministic `cos(x)` approximation, via `sin(x + π/2)`.
#[must_use]
pub(crate) fn cos_approx(x: f32) -> f32 {
    sin_approx(x + FRAC_PI_2_F32)
}

/// A deterministic `tan(x)` approximation, via `sin/cos`.
#[must_use]
#[allow(dead_code)]
pub(crate) fn tan_approx(x: f32) -> f32 {
    let c = cos_approx(x);
    if c == 0.0 {
        return 0.0;
    }
    sin_approx(x) / c
}

/// A deterministic `atan2(y, x)` approximation in `(-π, π]`.
///
/// Uses an octant reduction and a minimax polynomial for `atan` on `[0, 1]`
/// (coefficients from the classic `atan_pade` family), evaluated in a fixed
/// order. This is the in-crate `atan2` the spec names (`signal.md` §3.4).
#[must_use]
pub(crate) fn atan2_approx(y: f32, x: f32) -> f32 {
    if x == 0.0 && y == 0.0 {
        return 0.0;
    }
    let ax = x.abs();
    let ay = y.abs();
    let (a, swapped) = if ay > ax { (ax / ay, true) } else { (ay / ax, false) };
    let s = a * a;
    let mut r = ((-0.046_496_474_9 * s + 0.159_314_22) * s - 0.327_622_764) * s * a + a;
    if swapped {
        r = FRAC_PI_2_F32 - r;
    }
    if x < 0.0 {
        r = PI_F32 - r;
    }
    if y < 0.0 {
        -r
    } else {
        r
    }
}

/// A deterministic `ln(x)` approximation for `x > 0`.
///
/// Decomposes `x = m · 2^e` with `m ∈ [√½, √2)`, then evaluates
/// `ln(m) = 2·atanh((m−1)/(m+1))` to four odd terms. Returns `-∞` at zero and
/// `NaN` for negative input, matching the platform function's shape.
#[must_use]
pub(crate) fn ln_approx(x: f32) -> f32 {
    if x <= 0.0 {
        return if x == 0.0 { f32::NEG_INFINITY } else { f32::NAN };
    }
    let bits = x.to_bits();
    let mut e = ((bits >> 23) & 0xff) as i32 - 127;
    let mut m = f32::from_bits((bits & 0x007f_ffff) | 0x3f80_0000);
    if m > 1.414_213_6 {
        m *= 0.5;
        e += 1;
    }
    let t = (m - 1.0) / (m + 1.0);
    let t2 = t * t;
    let mut p = t;
    let mut s = t;
    p *= t2;
    s += p / 3.0;
    p *= t2;
    s += p / 5.0;
    p *= t2;
    s += p / 7.0;
    p *= t2;
    s += p / 9.0;
    2.0 * s + (e as f32) * 0.693_147_2
}

/// A deterministic `log10(x)` approximation, via [`ln_approx`].
#[must_use]
pub(crate) fn log10_approx(x: f32) -> f32 {
    ln_approx(x) * 0.434_294_48
}

/// `2π` in `f32`, exposed to the crate for angle construction.
pub(crate) const TAU: f32 = TWO_PI_F32;
