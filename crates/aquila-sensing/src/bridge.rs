//! The contract ↔ signal-representation bridge.
//!
//! `aquila-sensing` is the boundary where the one canonical contract frame
//! (`aquila_types::CsiFrame`: a raw signed int8 I/Q sample, imaginary-first)
//! meets `aquila-signal`'s internal conditioning representation
//! (`aquila_signal::ComplexFrame`: per-subcarrier `Complex32` bins). The
//! conversion itself lives in `aquila-signal` as `decode_csi`; this module is
//! the thin envelope-mapping adapter so `aquila-signal` need not know about
//! `CsiEnvelope` (`signal.md` §2).
//!
//! A malformed contract frame is surfaced as a typed [`DecodeError`], never
//! silently repaired: the contract frame is validated on ingest, so an error
//! here means the frame should never have existed.

use aquila_signal::{ComplexFrame, DecodeError};
use aquila_types::CsiEnvelope;

/// Bridge one contract frame into `aquila-signal`'s conditioning
/// representation.
///
/// # Errors
///
/// Returns [`DecodeError::EmptyFrame`] when the contract frame carries no
/// subcarriers, or [`DecodeError::OddIqLength`] when its interleaved I/Q sample
/// has an odd length.
pub fn to_signal_frame(frame: &aquila_types::CsiFrame) -> Result<ComplexFrame, DecodeError> {
    aquila_signal::decode_csi(frame)
}

/// Bridge a slice of core envelopes into a signal chunk.
///
/// # Errors
///
/// Returns [`DecodeError`] on the first malformed contract frame.
pub fn to_signal_chunk(frames: &[CsiEnvelope]) -> Result<Vec<ComplexFrame>, DecodeError> {
    frames.iter().map(|env| to_signal_frame(&env.frame)).collect()
}
