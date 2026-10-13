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
//! Two specification sections describe the CSI data plane. `capture-and-firmware.md`
//! §5.1 names it `AQCI` with a 56-byte header carrying a factory MAC and
//! `int8`-interleaved I/Q; `data-contracts.md` §9.1 calls the *canonical on-air*
//! format `AQCS` with a 16-byte header and `f32` amplitude/phase arrays. The two
//! conflict, and §9.1 is the only one that round-trips the frozen `CsiFrame`
//! shape (`data-contracts.md` §2/§8) through the required
//! `encode_datagram`/`decode_datagram` signatures.
//!
//! This crate therefore adopts the **§9.1 canonical layout** — the header and
//! body offsets, the major/minor version rules and the `f32` payload — under
//! the **`AQCI` magic** that the task and §5.1 name. The divergences from §9.1
//! are the magic (`AQCI` not `AQCS`) and the identity field (the dense
//! [`NodeId`](aquila_types::NodeId) that `CsiFrame` carries, not a MAC). The
//! MAC↔`NodeId` translation that `ADR-0003` §3 places at the seam is provided
//! by [`NodeIdentityMap`], which `aquila-sources` maintains from the topology.
//! This is recorded as the crate's pinned layout decision.
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
//!     amplitude: vec![1.0, 2.0, 3.0],
//!     phase: vec![0.0, -0.5, 0.5],
//!     rssi_dbm: -40,
//!     noise_floor_dbm: -90,
//!     phy: PhyFlags(PhyFlags::HT),
//! };
//!
//! let bytes = encode_datagram(&frame);
//! assert_eq!(decode_datagram(&bytes).unwrap(), frame);
//! ```
//!
//! Australian English throughout (serialisation, behaviour, normalise).

mod c_header;
mod constants;
mod control;
mod cursor;
mod datagram;
mod error;
mod identity;

#[cfg(test)]
mod tests;

pub use c_header::generate_c_header;
pub use constants::{
    Opcode, CONTROL_HEADER_LEN, CONTROL_MAGIC, CONTROL_PORT, CONTROL_VERSION, CSI_BODY_FIXED_LEN,
    CSI_DATA_PORT, CSI_HEADER_LEN, CSI_MAGIC, CSI_MESSAGE_TYPE, MAX_CONTROL_STRING,
    MAX_DATAGRAM_LEN, MAX_SOURCE_MACS, MAX_SUBCARRIERS, SUBCARRIER_FORMAT_F32, WIRE_MAJOR,
    WIRE_MINOR,
};
pub use control::{
    decode_control, encode_control, message_mac, result_message, Ack, AckStatus, ControlBody,
    ControlMessage, EventNotice, JoinAck, JoinRequest, SetConfig, SetCsi, Status, SyncEcho,
    SyncPing,
};
pub use datagram::{datagram_len, decode_datagram, encode_datagram};
pub use error::{DecodeError, WireError};
pub use identity::NodeIdentityMap;
