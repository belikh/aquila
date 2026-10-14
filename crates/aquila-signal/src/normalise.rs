//! Hardware normalisation.
//!
//! `normalise` removes the per-node, per-subcarrier gain and phase response from
//! the global calibration (`signal.md` §4.4). The runtime hot path is a complex
//! multiply — calibration stores complex multipliers, so no `sin`/`cos` runs
//! here.

use crate::decode::{Complex32, ComplexFrame, FrameFlags, RxMeta};

/// The calibration state carried on a normalised frame (`N5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CalibrationState {
    /// A complete, finite multiplier array was supplied.
    Calibrated,
    /// No multiplier array was supplied; the identity was applied.
    #[default]
    Uncalibrated,
    /// Some multipliers were missing or non-finite; identity was applied there.
    Partial,
}

/// Per-node hardware normalisation, precomputed.
///
/// `per_node` has length `expected_bins`; an empty array means the identity
/// (uncalibrated). Runtime is a pure complex multiply.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Calibration {
    /// The per-subcarrier complex multipliers; identity if empty.
    pub per_node: Vec<Complex32>,
    /// The calibration state carried onto every normalised frame.
    pub state: CalibrationState,
}

/// A normalised frame: the calibrated bins plus the carried metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalisedFrame {
    /// The receive metadata, carried verbatim.
    pub meta: RxMeta,
    /// The calibrated complex coefficients.
    pub bins: Vec<Complex32>,
    /// The decode flags, carried verbatim.
    pub flags: FrameFlags,
    /// The calibration state actually applied.
    pub calibration: CalibrationState,
    /// The number of bins whose multiplier was non-finite and treated as
    /// identity, so `NaN` is never propagated.
    pub nonfinite_multipliers: usize,
}

/// Apply the hardware calibration to one frame (`N1`–`N5`).
///
/// `H_norm[i] = H[i] · C[i]`. An empty `per_node`, or an exact `1 + 0i`
/// multiplier, is an identity copy and is bit-exact (`N1`). A non-finite
/// multiplier is treated as identity for that bin and counted, never
/// propagated. A length mismatch leaves the frame uncalibrated and records
/// [`CalibrationState::Partial`]; callers that want a hard failure validate the
/// shape first (see [`process`](crate::process)).
#[must_use]
pub fn normalise(frame: &ComplexFrame, cal: &Calibration) -> NormalisedFrame {
    if cal.per_node.is_empty() {
        return NormalisedFrame {
            meta: frame.meta,
            bins: frame.bins.clone(),
            flags: frame.flags,
            calibration: cal.state,
            nonfinite_multipliers: 0,
        };
    }

    let mut nonfinite = 0;
    let mut bins = Vec::with_capacity(frame.bins.len());
    for (i, bin) in frame.bins.iter().enumerate() {
        match cal.per_node.get(i) {
            None => {
                // Missing multiplier: identity, and the result is partial.
                nonfinite += 1;
                bins.push(*bin);
            }
            Some(c) if !c.is_finite() => {
                nonfinite += 1;
                bins.push(*bin);
            }
            Some(c) if *c == Complex32::ONE => {
                bins.push(*bin);
            }
            Some(c) => {
                bins.push(*bin * *c);
            }
        }
    }

    let state = if nonfinite == 0 && cal.per_node.len() == frame.bins.len() {
        cal.state
    } else {
        CalibrationState::Partial
    };

    NormalisedFrame {
        meta: frame.meta,
        bins,
        flags: frame.flags,
        calibration: state,
        nonfinite_multipliers: nonfinite,
    }
}
