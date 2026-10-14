#![forbid(unsafe_code)]

//! `aquila-signal` — CSI signal conditioning.
//!
//! This crate is level 1 in the workspace dependency graph and depends only on
//! [`aquila_types`]. It is one deep, pure module: it turns a stream of recorded
//! CSI frames into per-node signal features (presence energy, motion residuals,
//! spectrograms, breathing/heart-rate candidates). Everything here is a pure
//! function over owned data — no IO, no clocks, no globals, no threads, and a
//! fixed accumulation order (`signal.md` §1, §3).
//!
//! # Determinism
//!
//! Bit-exactness is a first-class requirement. The reference path calls no
//! platform `sin`/`cos`/`tan`/`atan2`/`log` (`signal.md` §3.4, `G4`); where a
//! stage needs a transcendental it uses the pinned polynomial approximations in
//! the crate's `math` module. Length-sensitive sums accumulate in `f64` and
//! narrow once; variance uses a stable two-pass form. Identical
//! `(frames, Calibration, Config)` therefore yields bit-identical
//! [`Features`], which [`Features::fingerprint`] can verify byte-for-byte.
//!
//! # Relationship to the contracts
//!
//! The condition-stage types here ([`ComplexFrame`], [`Complex32`], [`RxMeta`],
//! [`SubcarrierLayout`], [`Calibration`]) are the signal pipeline's **internal**
//! working representation, declared by `signal.md` §2. Node identity reuses the
//! frozen [`aquila_types::NodeId`] rather than forking it. The one canonical
//! contract frame is `aquila_types::CsiFrame` (amplitude/phase arrays,
//! `data-contracts.md` §2); [`decode_csi`] is the single conversion from it into
//! this crate's [`ComplexFrame`], so there is no second `CsiFrame`.
//!
//! # Stage functions
//!
//! [`decode_csi`] → [`normalise`] → [`unwrap_phase`] / [`sanitise_phase`] →
//! [`bandpass`] / [`resonator`] → [`motion_features`] → [`stft`] /
//! [`bvp_track`] → [`resample_uniform`] → [`select_subcarriers`], composed by
//! [`process`]. [`validate_sequence`] surfaces transport reality as data.
//!
//! # Prose
//!
//! Australian English throughout (normalise, sanitise, colour, behaviour).

mod decode;
mod error;
mod filter;
mod math;
mod motion;
mod normalise;
mod phase;
mod process;
mod resample;
mod selection;
mod spectrogram;
mod vitals;

#[cfg(test)]
mod tests;

pub use aquila_types::NodeId;
pub use decode::{
    decode_csi, decode_record, Bandwidth, Complex32, ComplexFrame, FrameFlags, RxMeta,
    SubcarrierLayout,
};
pub use error::{ConfigError, DecodeError, ResampleError, SignalError};
pub use filter::{bandpass, resonator, Band, Biquad, BiquadCoeffs};
pub use motion::{motion_features, rms};
pub use normalise::{normalise, Calibration, CalibrationState, NormalisedFrame};
pub use phase::{sanitise_phase, unwrap_boundary, unwrap_phase, SanitisedPhase};
pub use process::{process, validate_sequence, Config, Features, NodeFeatures, SeqAnomaly};
pub use resample::resample_uniform;
pub use selection::{select_subcarriers, SubcarrierFeatureMatrix, SubcarrierMask};
pub use spectrogram::{bvp_track, stft, BvpSample, Spectrogram, StftConfig};
pub use vitals::{
    estimate_band, estimate_vitals, VitalEstimate, VitalsConfig, VitalsEstimates, VitalsParams,
};
