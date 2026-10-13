//! The contract ↔ signal-representation bridge.
//!
//! `aquila-sensing` is the one place where the on-wire contract CSI frame
//! (`aquila_types::CsiFrame`: ordered `amplitude`/`phase` arrays) meets
//! `aquila-signal`'s conditioning representation (`aquila_signal::CsiFrame`:
//! per-subcarrier `Complex32` bins). The bridge is a pure function and lives
//! here so `aquila-signal` stays untouched and unaware of the transport types
//! (`signal.md` §2).
//!
//! The reconstruction `re = a·cos(φ)`, `im = a·sin(φ)` inverts the magnitude and
//! argument the wire format carries. A non-finite amplitude or phase is mapped
//! to zero rather than propagated, so a malformed frame can never inject `NaN`
//! into the DSP.

use aquila_signal::{Bandwidth, Complex32, CsiFrame as SignalFrame, FrameFlags, RxMeta};
use aquila_types::{Band, CsiEnvelope, CsiFrame as ContractFrame};

/// The nominal centre frequency of a channel, in hertz, for diagnostics only.
fn centre_freq_hz(band: Band, channel: u8) -> u32 {
    match band {
        Band::B2G4 => 2_407 + u32::from(channel) * 5,
        Band::B5G => 5_000 + u32::from(channel) * 5,
        Band::Unknown => 0,
    }
}

/// Bridge one contract frame into `aquila-signal`'s conditioning representation.
///
/// The subcarrier count is taken from `amplitude.len()`; a mismatch with the
/// phase array is handled by using the shorter length, so the bridge never
/// panics on a malformed frame.
#[must_use]
pub fn to_signal_frame(frame: &ContractFrame) -> SignalFrame {
    let n = frame.amplitude.len().min(frame.phase.len());
    let mut bins = Vec::with_capacity(n);
    for i in 0..n {
        let a = frame.amplitude[i];
        let p = frame.phase[i];
        if a.is_finite() && p.is_finite() {
            bins.push(Complex32::new(a * p.cos(), a * p.sin()));
        } else {
            bins.push(Complex32::ZERO);
        }
    }

    let bandwidth = if frame.bandwidth_mhz >= 40 {
        Bandwidth::Ht40
    } else {
        Bandwidth::Ht20
    };

    let meta = RxMeta {
        node: frame.node_id,
        seq: frame.sequence.min(u32::MAX as u64) as u32,
        t_device_us: frame.device_time_us,
        centre_freq_hz: centre_freq_hz(frame.band, frame.channel),
        bandwidth,
        rssi_dbm: frame.rssi_dbm,
        noise_floor_dbm: frame.noise_floor_dbm,
        first_word_invalid: false,
    };

    SignalFrame {
        meta,
        bins,
        flags: FrameFlags::default(),
    }
}

/// Bridge a slice of core envelopes into a signal chunk.
#[must_use]
pub fn to_signal_chunk(frames: &[CsiEnvelope]) -> Vec<SignalFrame> {
    frames
        .iter()
        .map(|env| to_signal_frame(&env.frame))
        .collect()
}
