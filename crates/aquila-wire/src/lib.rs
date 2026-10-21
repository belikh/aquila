#![forbid(unsafe_code)]

//! `aquila-wire` — the single on-air codec for Aquila.
//!
//! This crate is level 1 in the workspace dependency graph and depends only on
//! [`aquila_types`]. It owns the one wire layout for both node planes
//! (`capture-and-firmware.md` §5–6, `data-contracts.md` §9.1) and is the shared
//! codec beneath the `CsiSource`/`NodeControl` seam so `aquila-session`,
//! `aquila-simulator` and the C header generator need not depend on the seam
//! crate (`ADR-0003` §4).
//!
//! Two planes, two magics, two ports:
//!
//! - **CSI data plane** — [`encode_datagram`]/[`decode_datagram`], one
//!   [`CsiFrame`](aquila_types::CsiFrame) per datagram, `AQCI`, UDP
//!   [`CSI_DATA_PORT`] (9100).
//! - **Node control plane** — [`encode_control`]/[`decode_control`], `AQCC`
//!   request/response messages, UDP [`CONTROL_PORT`] (9101).
//!
//! Decoding never panics: every short buffer, wrong magic, wrong version or
//! inconsistent length is a typed [`WireError`]. The CSI decoder tolerates a
//! minor-version bump and any bytes past the declared payload, so newer node
//! firmware is forward-compatible with an older core.
//!
//! # Layout decision
//!
//! The CSI data plane is one layout under the `AQCI` magic that
//! `capture-and-firmware.md` §5.1 and the interface register name. The header
//! and body offsets are those of `data-contracts.md` §9.1; the payload is the
//! raw signed int8 I/Q sample the esp-csi capture produced — the verbatim
//! sample is ADR-0005, the imaginary-first byte order is ADR-0006 — two bytes
//! per subcarrier, not a derived `f32` amplitude/phase pair. The identity
//! field is the dense [`NodeId`](aquila_types::NodeId) that `CsiFrame` carries,
//! not a MAC; the MAC↔`NodeId` translation that `ADR-0003` §3 places at the
//! seam is provided by [`NodeIdentityMap`], which `aquila-sources` maintains
//! from the topology.
//!
//! # Example
//!
//! ```
//! use aquila_types::{Band, CsiFrame, NodeId, PhyFlags};
//! use aquila_wire::{decode_datagram, encode_datagram};
//!
//! let frame = CsiFrame {
//!     node_id: NodeId(7),
//!     boot_epoch: 1,
//!     sequence: 42,
//!     device_time_us: 100_000,
//!     band: Band::B2G4,
//!     channel: 6,
//!     bandwidth_mhz: 20,
//!     subcarrier_offset: -32,
//!     i_q: vec![10, -3, 20, 0, -30, 15],
//!     rssi_dbm: -40,
//!     noise_floor_dbm: -90,
//!     phy: PhyFlags(PhyFlags::HT),
//!     first_word_invalid: false,
//!     source_mac: None,
//!     rx_seq: None,
//!     rx_flags: 0,
//!     rx_timestamp: 0,
//! };
//!
//! let bytes = encode_datagram(&frame);
//! assert_eq!(decode_datagram(&bytes).unwrap(), frame);
//! ```
//!
//! Australian English throughout (serialisation, behaviour, normalise).

mod auth;
mod c_header;
mod constants;
mod control;
mod cursor;
mod datagram;
mod error;
mod identity;

#[cfg(test)]
mod tests;

pub use auth::{control_auth_tag, verify_control_auth};
pub use c_header::generate_c_header;
pub use constants::{
    Opcode, CONTROL_AUTH_LEN, CONTROL_HEADER_LEN, CONTROL_MAGIC, CONTROL_PORT, CONTROL_VERSION,
    CSI_BODY_FIXED_LEN, CSI_DATA_PORT, CSI_FLAG_FIRST_WORD_INVALID, CSI_HEADER_LEN, CSI_MAGIC,
    CSI_MESSAGE_TYPE, MAX_CONTROL_STRING, MAX_DATAGRAM_LEN, MAX_SOURCE_MACS, MAX_SUBCARRIERS,
    OTA_DEFAULT_CHUNK_SIZE, OTA_DEFAULT_WINDOW, OTA_MAX_CHUNK_SIZE, SUBCARRIER_FORMAT_INT8_IQ,
    WIRE_MAJOR, WIRE_MINOR,
};
pub use control::{
    decode_control, decode_control_authenticated, encode_control, encode_control_authenticated,
    message_mac, result_message, Ack, AckStatus, ControlBody, ControlMessage, EventNotice, JoinAck,
    JoinRequest, OtaAbort, OtaAck, OtaAckStatus, OtaBegin, OtaChunk, OtaEnd, OtaOutcome, OtaResult,
    OtaState, OtaStatus, SetConfig, SetCsi, Status, SyncEcho, SyncPing,
};
pub use datagram::{datagram_len, decode_datagram, encode_datagram};
pub use error::{DecodeError, WireError};
pub use identity::NodeIdentityMap;
