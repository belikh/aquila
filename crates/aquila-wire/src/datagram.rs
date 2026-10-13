//! The CSI data plane: one `CsiFrame` per UDP datagram, little-endian.
//!
//! The layout is the canonical on-air format of `data-contracts.md` §9.1, which
//! maps one-for-one onto the frozen [`CsiFrame`] shape, carried under the
//! `AQCI` magic named by `capture-and-firmware.md` §5.1 and the interface
//! register. See the crate-level *Layout decision* note for the reconciliation.
//!
//! ```text
//! offset size field
//!      0    4 magic = "AQCI"
//!      4    1 wire_major
//!      5    1 wire_minor
//!      6    1 msg_type = 1
//!      7    1 sc_format = 0
//!      8    2 node_id
//!     10    2 payload_len
//!     12    1 band
//!     13    1 channel
//!     14    1 bandwidth_mhz
//!     15    1 reserved = 0
//!     16    8 sequence
//!     24    8 device_time_us
//!     32    2 boot_epoch
//!     34    2 subcarrier_offset (i16)
//!     36    2 n_subcarriers
//!     38    1 rssi_dbm (i8)
//!     39    1 noise_floor_dbm (i8)
//!     40    2 phy_flags
//!     42    2 reserved = 0
//!     44   4n amplitude[0..n] (f32)
//!  44+4n   4n phase[0..n] (f32)
//! ```

use aquila_types::{Band, CsiFrame, NodeId, PhyFlags};

use crate::constants::{
    CSI_BODY_FIXED_LEN, CSI_HEADER_LEN, CSI_MAGIC, CSI_MESSAGE_TYPE, MAX_SUBCARRIERS,
    SUBCARRIER_FORMAT_F32, WIRE_MAJOR, WIRE_MINOR,
};
use crate::cursor::{Reader, Writer};
use crate::error::WireError;

/// The exact length in bytes of a CSI datagram carrying `n_subcarriers`.
///
/// Returns `None` when the count is zero or exceeds [`MAX_SUBCARRIERS`], so a
/// caller can reject an unencodable frame before calling [`encode_datagram`].
#[must_use]
pub fn datagram_len(n_subcarriers: usize) -> Option<usize> {
    if n_subcarriers == 0 || n_subcarriers > MAX_SUBCARRIERS as usize {
        return None;
    }
    Some(CSI_HEADER_LEN + CSI_BODY_FIXED_LEN + 8 * n_subcarriers)
}

/// Map a [`Band`] to its on-wire byte.
const fn band_to_u8(band: Band) -> u8 {
    match band {
        Band::B2G4 => 0,
        Band::B5G => 1,
        Band::Unknown => 255,
    }
}

/// Map an on-wire band byte back to a [`Band`]; an unrecognised byte is
/// preserved as [`Band::Unknown`], never reinterpreted as a known band.
const fn band_from_u8(value: u8) -> Band {
    match value {
        0 => Band::B2G4,
        1 => Band::B5G,
        _ => Band::Unknown,
    }
}

/// Encode one [`CsiFrame`] as an `AQCI` datagram.
///
/// The function is total: it never fails and never panics. A frame whose
/// `amplitude` is longer than [`MAX_SUBCARRIERS`] is clamped to the wire budget
/// (such a frame could not cross the MTU anyway); a frame whose `phase` is
/// shorter than its `amplitude` is padded with `0.0`. Frames obeying the
/// [`CsiFrame`] invariants round-trip exactly.
#[must_use]
pub fn encode_datagram(frame: &CsiFrame) -> Vec<u8> {
    let n = frame.amplitude.len().min(MAX_SUBCARRIERS as usize);
    let payload_len = CSI_BODY_FIXED_LEN + 8 * n;

    let mut w = Writer::new();
    // Fixed header.
    w.bytes(&CSI_MAGIC);
    w.u8(WIRE_MAJOR);
    w.u8(WIRE_MINOR);
    w.u8(CSI_MESSAGE_TYPE);
    w.u8(SUBCARRIER_FORMAT_F32);
    w.u16(frame.node_id.0);
    w.u16(payload_len as u16);
    w.u8(band_to_u8(frame.band));
    w.u8(frame.channel);
    w.u8(frame.bandwidth_mhz);
    w.u8(0); // reserved

    // Fixed body.
    w.u64(frame.sequence);
    w.u64(frame.device_time_us);
    w.u16(frame.boot_epoch);
    w.i16(frame.subcarrier_offset);
    w.u16(n as u16);
    w.i8(frame.rssi_dbm);
    w.i8(frame.noise_floor_dbm);
    w.u16(frame.phy.0);
    w.u16(0); // reserved

    // Amplitude, then phase.
    for i in 0..n {
        w.f32(frame.amplitude[i]);
    }
    for i in 0..n {
        w.f32(frame.phase.get(i).copied().unwrap_or(0.0));
    }

    w.into_vec()
}

/// Decode one `AQCI` datagram into a [`CsiFrame`].
///
/// Validates the magic, the wire major version, the message type, the
/// subcarrier format, the subcarrier count and the internal length consistency
/// before trusting any field. A minor-version mismatch and any bytes after the
/// declared payload are accepted, so a newer node firmware is forward-compatible
/// with an older core. Never panics.
pub fn decode_datagram(bytes: &[u8]) -> Result<CsiFrame, WireError> {
    let mut r = Reader::new(bytes);

    // Fixed header.
    let magic = r.array::<4>()?;
    if magic != CSI_MAGIC {
        return Err(WireError::BadMagic {
            expected: CSI_MAGIC,
            found: magic,
        });
    }
    let major = r.u8()?;
    let _minor = r.u8()?; // additive: any minor is accepted
    if major != WIRE_MAJOR {
        return Err(WireError::UnsupportedVersion {
            major,
            expected: WIRE_MAJOR,
        });
    }
    let msg_type = r.u8()?;
    if msg_type != CSI_MESSAGE_TYPE {
        return Err(WireError::UnknownMessageType(msg_type));
    }
    let sc_format = r.u8()?;
    if sc_format != SUBCARRIER_FORMAT_F32 {
        return Err(WireError::UnknownSubcarrierFormat(sc_format));
    }
    let node_id = NodeId(r.u16()?);
    let payload_len = r.u16()? as usize;
    let band = band_from_u8(r.u8()?);
    let channel = r.u8()?;
    let bandwidth_mhz = r.u8()?;
    let _reserved = r.u8()?;

    // Fail early, with a clean truncation, if the declared payload is not all
    // present.
    let needed = CSI_HEADER_LEN + payload_len;
    if bytes.len() < needed {
        return Err(WireError::Truncated {
            needed,
            actual: bytes.len(),
        });
    }

    // Fixed body.
    let sequence = r.u64()?;
    let device_time_us = r.u64()?;
    let boot_epoch = r.u16()?;
    let subcarrier_offset = r.i16()?;
    let n = r.u16()?;
    if n == 0 || n > MAX_SUBCARRIERS {
        return Err(WireError::SubcarrierCount {
            count: n,
            max: MAX_SUBCARRIERS,
        });
    }

    let expected_payload = CSI_BODY_FIXED_LEN + 8 * n as usize;
    if payload_len != expected_payload {
        return Err(WireError::LengthMismatch {
            declared: payload_len,
            expected: expected_payload,
        });
    }

    let rssi_dbm = r.i8()?;
    let noise_floor_dbm = r.i8()?;
    let phy = PhyFlags(r.u16()?);
    let _body_reserved = r.u16()?;

    let count = n as usize;
    let mut amplitude = Vec::with_capacity(count);
    for _ in 0..count {
        amplitude.push(r.f32()?);
    }
    let mut phase = Vec::with_capacity(count);
    for _ in 0..count {
        phase.push(r.f32()?);
    }

    // Bytes beyond the declared payload are ignored (forward-compatible).
    Ok(CsiFrame {
        node_id,
        boot_epoch,
        sequence,
        device_time_us,
        band,
        channel,
        bandwidth_mhz,
        subcarrier_offset,
        amplitude,
        phase,
        rssi_dbm,
        noise_floor_dbm,
        phy,
    })
}
