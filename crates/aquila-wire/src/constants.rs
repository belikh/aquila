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
pub const WIRE_MINOR: u8 = 0;

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
pub const CSI_BODY_FIXED_LEN: usize = 28;

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
/// `(1472 - 16 - 28) / 2 = 714`.
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

/// The control opcodes of `capture-and-firmware.md` §6.2.
///
/// Each discriminant is the on-wire opcode byte.
///
/// The firmware-OTA extension opcodes `0x0E`–`0x14` are **specified but not
/// implemented in v1** (`firmware-ota.md`; tickets #13–#15) and are
/// deliberately absent here: defining them would imply a wire contract the node
/// and core do not yet honour (`sister-issues-firmware-ha.md` F2/F29/F36/F38).
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
    /// Reserved, zero (`u16`).
    pub(crate) const RESERVED: usize = 26;
    /// The first interleaved signed int8 byte: `Q` (imaginary first), then `I`.
    pub(crate) const IQ: usize = 28;
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
