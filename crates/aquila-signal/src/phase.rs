//! Phase unwrapping and sanitisation.
//!
//! Unwrapping makes per-subcarrier phase continuous (`signal.md` §4.5);
//! sanitisation removes the linear sampling-time-offset slope and the constant
//! offset across subcarriers, leaving the residual that carries motion and
//! vital signs (`signal.md` §4.6). Both are deterministic and use only
//! comparisons, integer multiples of `2π` and fixed-order `f64` accumulation.

use crate::decode::SubcarrierLayout;
use crate::math::TAU;

/// The residual phase after removing the affine slope across subcarriers.
#[derive(Debug, Clone, PartialEq)]
pub struct SanitisedPhase {
    /// The residual phase, one value per input sample.
    pub residual: Vec<f32>,
    /// The fitted slope, in radians per subcarrier index.
    pub slope: f32,
    /// The fitted intercept, in radians.
    pub intercept: f32,
    /// Whether a valid affine fit was possible (`P3`, `P6`).
    pub fit_ok: bool,
}

/// Unwrap a wrapped phase series so adjacent differences lie in `(−π, π]`.
///
/// `u[0] = φ[0]` and `u[k] = φ[k] + 2π·m[k]` with `m[k]` chosen so
/// `u[k] − u[k−1] ∈ (−π, π]` (`U1`–`U4`). Only comparisons and integer
/// multiples of `2π` are used, so the result is bit-deterministic. An empty
/// input yields an empty output.
///
/// `U5` (boundary honesty) is surfaced separately by
/// [`unwrap_boundary`], which reports whether any pre-correction jump exceeded
/// `π`.
#[must_use]
pub fn unwrap_phase(wrapped: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(wrapped.len());
    let Some((&first, rest)) = wrapped.split_first() else {
        return out;
    };
    out.push(first);
    let mut previous = first;
    for &phi in rest {
        let mut delta = phi - previous;
        if delta > core::f32::consts::PI {
            delta -= TAU;
        } else if delta <= -core::f32::consts::PI {
            delta += TAU;
        }
        let u = out[out.len() - 1] + delta;
        out.push(u);
        previous = phi;
    }
    out
}

/// Report whether any adjacent wrapped jump exceeded `π` (`U5`).
///
/// Such a jump is a possible null-bin boundary and its correction is not
/// silently trusted. Empty or single-element inputs report `false`.
#[must_use]
pub fn unwrap_boundary(wrapped: &[f32]) -> bool {
    wrapped
        .windows(2)
        .any(|w| (w[1] - w[0]).abs() > core::f32::consts::PI)
}

/// Remove the affine phase slope and offset across the usable bins.
///
/// The least-squares line is fitted in a fixed `f64` accumulation order
/// (`signal.md` §4.6):
///
/// ```text
/// κ̄ = Σk_i / n,   φ̄ = Σu_i / n
/// a  = Σ(k_i − κ̄)(u_i − φ̄) / Σ(k_i − κ̄)²
/// b  = φ̄ − a·κ̄
/// residual[i] = u_i − (a·k_i + b)
/// ```
///
/// Requires `n ≥ 2` and finite input; otherwise returns `fit_ok = false` and
/// the input unchanged (`P3`, `P6`). Satisfies affine equivariance (`P1`),
/// orthogonality of the residual to `{1, k}` (`P2`) and shift covariance
/// (`P5`).
#[must_use]
pub fn sanitise_phase(unwrapped: &[f32], layout: &SubcarrierLayout) -> SanitisedPhase {
    let n = unwrapped.len().min(layout.usable.len());
    if n < 2 || unwrapped.iter().any(|v| !v.is_finite()) {
        return SanitisedPhase {
            residual: unwrapped.to_vec(),
            slope: 0.0,
            intercept: 0.0,
            fit_ok: false,
        };
    }

    let k: Vec<f64> = layout.usable[..n].iter().map(|&i| f64::from(i)).collect();
    let u: Vec<f64> = unwrapped[..n].iter().map(|&v| f64::from(v)).collect();

    let nf = n as f64;
    let mut sum_k = 0.0_f64;
    let mut sum_u = 0.0_f64;
    for i in 0..n {
        sum_k += k[i];
        sum_u += u[i];
    }
    let k_bar = sum_k / nf;
    let u_bar = sum_u / nf;

    let mut s_kk = 0.0_f64;
    let mut s_ku = 0.0_f64;
    for i in 0..n {
        let dk = k[i] - k_bar;
        let du = u[i] - u_bar;
        s_kk += dk * dk;
        s_ku += dk * du;
    }

    if s_kk == 0.0 {
        return SanitisedPhase {
            residual: unwrapped.to_vec(),
            slope: 0.0,
            intercept: 0.0,
            fit_ok: false,
        };
    }

    let a = s_ku / s_kk;
    let b = u_bar - a * k_bar;

    let mut residual = Vec::with_capacity(unwrapped.len());
    for i in 0..n {
        residual.push((u[i] - (a * k[i] + b)) as f32);
    }
    for &extra in &unwrapped[n..] {
        residual.push(extra);
    }

    SanitisedPhase {
        residual,
        slope: a as f32,
        intercept: b as f32,
        fit_ok: true,
    }
}
