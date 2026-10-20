//! Codec constants: magics, versions, ports, size limits, opcodes and the exact
//! byte offsets of every header field.
//!
//! These are the single source of truth for both the Rust codec and the
//! generated `node_wire.h` (`capture-and-firmware.md` §9.4), so the two
//! languages cannot silently drift.

/// The CSI data-plane magic, ASCII `AQCI` (`41 51 43 49`).
///
/// `capture-and-firmware.md` §5.1, the interface register and
/// `data-contracts.md` §9.1 all name the single canonical data-plane body
/// `AQCI`; there is no second spelling. See the crate-level *Layout decision*
/// note.
pub const CSI_MAGIC: [u8; 4] = *b"AQCI";

/// The node control-plane magic, ASCII `AQCC` (`41 51 43 43`).
pub const CONTROL_MAGIC: [u8; 4] = *b"AQCC";

/// The wire major version. A datagram with a different major is rejected.
pub const WIRE_MAJOR: u8 = 1;

/// The wire minor version. A datagram with a different minor is accepted
/// (additive change within a major version).
///
/// The minor versions, and what the fixed body carries:
///
/// * **minor 0** — no `source_mac`, no `rx_seq`; the fixed body is 28 bytes
///   and the `u16` at body offset 26 is reserved.
/// * **minor 1** — `source_mac` at body offset 28; the fixed body is 34 bytes
///   and the `u16` at body offset 26 is still reserved.
/// * **minor 2** — `source_mac` at body offset 28 *and* `rx_seq` at body
///   offset 26; the fixed body is 34 bytes.
///
/// The codec knows each minor's body layout, so a node on any of them is
/// read correctly. The split matters: minor 1 and minor 2 have the same body
/// length, so a codec that inferred the layout from the length would read a
/// minor 1 node's reserved field as a sequence number.
pub const WIRE_MINOR: u8 = 2;

/// The control-framing version byte (`capture-and-firmware.md` §6.1).
pub const CONTROL_VERSION: u8 = WIRE_MAJOR;

/// The only CSI message type: a `CsiFrame`.
pub const CSI_MESSAGE_TYPE: u8 = 1;

/// The only subcarrier format: interleaved signed int8 `Q, I` pairs
/// (imaginary first), two bytes per subcarrier.
///
/// Carrying the raw int8 sample verbatim is ADR-0005; the imaginary-first byte
/// order (`[Q0, I0, Q1, I1, …]`) is ADR-0006.
pub const SUBCARRIER_FORMAT_INT8_IQ: u8 = 1;

/// The CSI header flags byte's bit 0: the leading `I/Q` pair is unreliable.
///
/// The node nulls the first complex sample (two bytes) when it sets this; the
/// core repeats the nulling and records the flag when decoding.
pub const CSI_FLAG_FIRST_WORD_INVALID: u8 = 0x01;

/// The fixed CSI header length in bytes.
pub const CSI_HEADER_LEN: usize = 16;

/// The fixed part of the CSI body, before the `int8` I/Q payload, in bytes.
///
/// Wire minor 1 adds the six-byte source MAC address of the transmitting
/// station, so the fixed body is 28 + 6 = 34 bytes.
pub const CSI_BODY_FIXED_LEN: usize = 34;

/// The fixed part of the CSI body for wire minor 0, before the `int8` I/Q
/// payload, in bytes.
///
/// Minor 0 has no source MAC field. It is kept so the codec can still read a
/// minor 0 datagram from a node that has not been updated.
pub const CSI_BODY_FIXED_LEN_V0: usize = 28;

/// The default CSI data-plane UDP port (node → core).
pub const CSI_DATA_PORT: u16 = 9100;

/// The default node control-plane UDP port (core ↔ node).
pub const CONTROL_PORT: u16 = 9101;

/// The largest datagram the codec will carry, chosen to stay under a 1500-byte
/// MTU without fragmentation (`data-contracts.md` §9.1).
pub const MAX_DATAGRAM_LEN: usize = 1472;

/// The largest supported subcarrier count.
///
/// Derived from [`MAX_DATAGRAM_LEN`]: with interleaved signed int8 `Q, I`
/// (imaginary first, 2 bytes per subcarrier) the body budget is
/// `(1472 - 16 - 34) / 2 = 711`.
pub const MAX_SUBCARRIERS: u16 =
    ((MAX_DATAGRAM_LEN - CSI_HEADER_LEN - CSI_BODY_FIXED_LEN) / 2) as u16;

/// The fixed control header length in bytes.
pub const CONTROL_HEADER_LEN: usize = 12;

/// The length of the AQCC control-plane authentication trailer in bytes.
///
/// An authenticated control message is the `AQCC` header and body followed by
/// this HMAC-SHA256 tag, keyed by the per-node pairing token
/// (`capture-and-firmware.md` §6.3). The plain framing is unchanged when the
/// trailer is absent, so the tag is purely additive.
pub const CONTROL_AUTH_LEN: usize = 32;

/// The maximum length of a length-prefixed control string (a one-byte prefix).
pub const MAX_CONTROL_STRING: usize = u8::MAX as usize;

/// The maximum number of source MACs in a config allow-list (a one-byte count).
pub const MAX_SOURCE_MACS: usize = u8::MAX as usize;

/// The default firmware-OTA chunk payload in bytes (`firmware-ota.md` §5).
pub const OTA_DEFAULT_CHUNK_SIZE: u16 = 1024;

/// The default firmware-OTA sliding-window depth (`firmware-ota.md` §5).
pub const OTA_DEFAULT_WINDOW: u8 = 8;

/// The largest firmware-OTA chunk payload the control datagram can carry.
///
/// The OTA body is `offset(4) + len(2) + crc32(4) + bytes` inside the fixed
/// 12-byte control header and the 32-byte auth trailer, so a chunk must fit
/// within [`MAX_DATAGRAM_LEN`].
pub const OTA_MAX_CHUNK_SIZE: u16 =
    (MAX_DATAGRAM_LEN - CONTROL_HEADER_LEN - 10 - CONTROL_AUTH_LEN) as u16;

/// The control opcodes of `capture-and-firmware.md` §6.2, including the
/// firmware-OTA extension `0x0E`–`0x14` of `firmware-ota.md` §5.
///
/// Each discriminant is the on-wire opcode byte. Ticket #13 adds the OTA
/// opcodes to the codec; the *firmware* agent that honours them is ticket #14,
/// so the codec defines the wire contract the core now speaks and a future node
/// will parse (one layout, two languages).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Opcode {
    /// `0x01` node → core: bootstrap pairing.
    JoinRequest = 0x01,
    /// `0x02` core → node: pairing accepted; token and config.
    JoinAck = 0x02,
    /// `0x03` both: reply to a `request_id`.
    Result = 0x03,
    /// `0x04` node → core: heartbeat (unsolicited).
    Status = 0x04,
    /// `0x05` core → node: read current config.
    GetConfig = 0x05,
    /// `0x06` core → node: apply config (transactional).
    SetConfig = 0x06,
    /// `0x07` core → node: enable or disable CSI streaming.
    SetCsi = 0x07,
    /// `0x08` core → node: time-sync probe carrying `t1`.
    SyncPing = 0x08,
    /// `0x09` node → core: time-sync echo carrying `t1`, `t2` and `t3`.
    SyncEcho = 0x09,
    /// `0x0A` core → node: reboot.
    Reboot = 0x0A,
    /// `0x0B` core → node: blink the LED.
    Identify = 0x0B,
    /// `0x0C` core → node: clear NVS and unpair.
    FactoryReset = 0x0C,
    /// `0x0D` node → core: a noteworthy unsolicited event.
    Event = 0x0D,
    /// `0x0E` core → node: start a firmware transfer (`firmware-ota.md` §5).
    OtaBegin = 0x0E,
    /// `0x0F` core → node: one image chunk.
    OtaChunk = 0x0F,
    /// `0x10` node → core: cumulative chunk acknowledgement.
    OtaAck = 0x10,
    /// `0x11` node → core: OTA progress/state.
    OtaStatus = 0x11,
    /// `0x12` core → node: finalise, verify and activate.
    OtaEnd = 0x12,
    /// `0x13` both: discard the transfer.
    OtaAbort = 0x13,
    /// `0x14` node → core: OTA outcome.
    OtaResult = 0x14,
}

impl Opcode {
    /// The on-wire opcode byte.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Parse an opcode byte, returning `None` for an unknown value.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::JoinRequest),
            0x02 => Some(Self::JoinAck),
            0x03 => Some(Self::Result),
            0x04 => Some(Self::Status),
            0x05 => Some(Self::GetConfig),
            0x06 => Some(Self::SetConfig),
            0x07 => Some(Self::SetCsi),
            0x08 => Some(Self::SyncPing),
            0x09 => Some(Self::SyncEcho),
            0x0A => Some(Self::Reboot),
            0x0B => Some(Self::Identify),
            0x0C => Some(Self::FactoryReset),
            0x0D => Some(Self::Event),
            0x0E => Some(Self::OtaBegin),
            0x0F => Some(Self::OtaChunk),
            0x10 => Some(Self::OtaAck),
            0x11 => Some(Self::OtaStatus),
            0x12 => Some(Self::OtaEnd),
            0x13 => Some(Self::OtaAbort),
            0x14 => Some(Self::OtaResult),
            _ => None,
        }
    }
}

/// Byte offsets within the fixed 16-byte CSI header (`data-contracts.md` §9.1).
pub(crate) mod csi_offset {
    /// ASCII `AQCI`.
    pub(crate) const MAGIC: usize = 0;
    /// Wire major version (`u8`).
    pub(crate) const WIRE_MAJOR: usize = 4;
    /// Wire minor version (`u8`).
    pub(crate) const WIRE_MINOR: usize = 5;
    /// Message type (`u8`).
    pub(crate) const MESSAGE_TYPE: usize = 6;
    /// Subcarrier format (`u8`).
    pub(crate) const SUBCARRIER_FORMAT: usize = 7;
    /// Node identity (`u16`).
    pub(crate) const NODE_ID: usize = 8;
    /// Payload length in bytes (`u16`).
    pub(crate) const PAYLOAD_LEN: usize = 10;
    /// Band (`u8`).
    pub(crate) const BAND: usize = 12;
    /// Channel (`u8`).
    pub(crate) const CHANNEL: usize = 13;
    /// Bandwidth in MHz (`u8`).
    pub(crate) const BANDWIDTH_MHZ: usize = 14;
    /// Flags byte (`u8`): bit 0 is [`super::CSI_FLAG_FIRST_WORD_INVALID`].
    pub(crate) const FLAGS: usize = 15;
}

/// Byte offsets within the fixed part of the CSI body.
pub(crate) mod csi_body_offset {
    /// Frame sequence (`u64`).
    pub(crate) const SEQUENCE: usize = 0;
    /// Device monotonic timestamp in microseconds (`u64`).
    pub(crate) const DEVICE_TIME_US: usize = 8;
    /// Boot epoch (`u16`).
    pub(crate) const BOOT_EPOCH: usize = 16;
    /// Subcarrier offset (`i16`).
    pub(crate) const SUBCARRIER_OFFSET: usize = 18;
    /// Subcarrier count (`u16`).
    pub(crate) const N_SUBCARRIERS: usize = 20;
    /// Whole-frame RSSI in dBm (`i8`).
    pub(crate) const RSSI_DBM: usize = 22;
    /// Whole-frame noise floor in dBm (`i8`).
    pub(crate) const NOISE_FLOOR_DBM: usize = 23;
    /// PHY flags (`u16`).
    pub(crate) const PHY_FLAGS: usize = 24;
    /// The 802.11 sequence number of the received frame (`u16`), wire minor 1
    /// and later; `0xFFFF` means the node did not report one.
    pub(crate) const RX_SEQ: usize = 26;
    /// The six-byte MAC address of the transmitting station (wire minor 1 and
    /// later).
    pub(crate) const SOURCE_MAC: usize = 28;
    /// The first interleaved signed int8 byte: `Q` (imaginary first), then `I`.
    pub(crate) const IQ: usize = 34;
}

/// Byte offsets within the fixed 12-byte control header
/// (`capture-and-firmware.md` §6.1).
pub(crate) mod control_offset {
    /// ASCII `AQCC`.
    pub(crate) const MAGIC: usize = 0;
    /// Version (`u8`).
    pub(crate) const VERSION: usize = 4;
    /// Opcode (`u8`).
    pub(crate) const OPCODE: usize = 5;
    /// Body length in bytes (`u16`).
    pub(crate) const BODY_LEN: usize = 6;
    /// Caller nonce echoed by replies (`u32`).
    pub(crate) const REQUEST_ID: usize = 8;
    /// The first body byte.
    pub(crate) const BODY: usize = 12;
}
