//! The node control plane: `AQCC` UDP request/response messages.
//!
//! A separate protocol from the CSI data plane (`capture-and-firmware.md` §6),
//! on its own port. Bodies are small fixed little-endian C-compatible structs,
//! not JSON or CBOR, so the firmware needs no parser. Every request is keyed by
//! `(node, request_id)`; a reply echoes the `request_id`.
//!
//! ```text
//! offset size field
//!      0    4 magic = "AQCC"
//!      4    1 version = 1
//!      5    1 opcode
//!      6    2 body_len
//!      8    4 request_id
//!     12    * body (opcode-specific)
//! ```

use aquila_types::MacAddr;

use crate::constants::{
    Opcode, CONTROL_HEADER_LEN, CONTROL_MAGIC, CONTROL_VERSION, MAX_SOURCE_MACS,
};
use crate::cursor::{Reader, Writer};
use crate::error::WireError;

/// A control message: a caller nonce plus an opcode-specific body.
///
/// A reply echoes the [`request_id`](ControlMessage::request_id) of the request
/// it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlMessage {
    /// Caller nonce; replies echo it so a request can be matched to its reply.
    pub request_id: u32,
    /// The opcode-specific body.
    pub body: ControlBody,
}

/// The status a `RESULT` (`0x03`) reply carries.
///
/// An unknown status byte is preserved as [`AckStatus::Unknown`], never
/// reinterpreted as a known status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AckStatus {
    /// The request was applied and is active.
    Active = 0,
    /// The request was refused; see the reply detail.
    Rejected = 1,
    /// An unrecognised status.
    Unknown = 255,
}

impl AckStatus {
    /// The on-wire status byte.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Map an on-wire status byte; unknown values become [`AckStatus::Unknown`].
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Active,
            1 => Self::Rejected,
            _ => Self::Unknown,
        }
    }
}

/// An opcode-specific control body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlBody {
    /// `JOIN_REQUEST` (`0x01`): bootstrap pairing, node → core.
    JoinRequest(JoinRequest),
    /// `JOIN_ACK` (`0x02`): pairing accepted, core → node.
    JoinAck(JoinAck),
    /// `RESULT` (`0x03`): a reply to a `request_id`, both directions.
    Result(Ack),
    /// `STATUS` (`0x04`): heartbeat, node → core.
    Status(Status),
    /// `GET_CONFIG` (`0x05`): read current config, core → node.
    GetConfig,
    /// `SET_CONFIG` (`0x06`): apply config, core → node.
    SetConfig(SetConfig),
    /// `SET_CSI` (`0x07`): enable or disable streaming, core → node.
    SetCsi(SetCsi),
    /// `SYNC_PING` (`0x08`): time-sync probe, core → node.
    SyncPing(SyncPing),
    /// `SYNC_ECHO` (`0x09`): time-sync echo, node → core.
    SyncEcho(SyncEcho),
    /// `REBOOT` (`0x0A`): reboot, core → node.
    Reboot,
    /// `IDENTIFY` (`0x0B`): blink the LED, core → node.
    Identify,
    /// `FACTORY_RESET` (`0x0C`): clear NVS and unpair, core → node.
    FactoryReset,
    /// `EVENT` (`0x0D`): a noteworthy unsolicited event, node → core.
    Event(EventNotice),
}

impl ControlBody {
    /// The opcode this body encodes to.
    #[must_use]
    pub const fn opcode(&self) -> Opcode {
        match self {
            Self::JoinRequest(_) => Opcode::JoinRequest,
            Self::JoinAck(_) => Opcode::JoinAck,
            Self::Result(_) => Opcode::Result,
            Self::Status(_) => Opcode::Status,
            Self::GetConfig => Opcode::GetConfig,
            Self::SetConfig(_) => Opcode::SetConfig,
            Self::SetCsi(_) => Opcode::SetCsi,
            Self::SyncPing(_) => Opcode::SyncPing,
            Self::SyncEcho(_) => Opcode::SyncEcho,
            Self::Reboot => Opcode::Reboot,
            Self::Identify => Opcode::Identify,
            Self::FactoryReset => Opcode::FactoryReset,
            Self::Event(_) => Opcode::Event,
        }
    }

    /// Encode this body, without its header.
    fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        match self {
            Self::JoinRequest(b) => {
                w.bytes(&b.node.0);
                w.u32(b.boot);
                w.u32(b.pairing_code);
                w.string_u8(&b.fw_version)?;
            }
            Self::JoinAck(b) => {
                w.bytes(&b.node.0);
                w.bytes(&b.token);
                w.u32(b.config_version);
                w.bytes(&b.config_hash);
            }
            Self::Result(b) => {
                w.u8(b.status.to_u8());
                w.string_u8(&b.detail)?;
            }
            Self::Status(b) => {
                w.u64(b.uptime_ms);
                w.u32(b.boot);
                w.u64(b.last_sequence);
                w.u32(b.dropped);
                w.u16(b.ring_depth);
                w.i16(b.temperature_c_x100);
                w.u32(b.config_version);
                w.u32(b.last_rtt_us);
            }
            Self::GetConfig | Self::Reboot | Self::Identify | Self::FactoryReset => {}
            Self::SetConfig(b) => {
                if b.source_mac_allowlist.len() > MAX_SOURCE_MACS {
                    return Err(WireError::TooManyMacs {
                        count: b.source_mac_allowlist.len(),
                        max: MAX_SOURCE_MACS,
                    });
                }
                w.u32(b.config_version);
                w.bytes(&b.config_hash);
                w.u8(b.channel);
                w.u8(b.bandwidth_mhz);
                w.u8(b.secondary);
                w.u8(u8::from(b.csi_enable));
                w.u8(u8::from(b.lltf_enable));
                w.u8(u8::from(b.ht_ltf_enable));
                w.u8(u8::from(b.stbc_ht_ltf_enable));
                w.u16(b.csi_len_cap);
                w.u32(b.max_tx_rate_hz);
                w.u32(b.heartbeat_ms);
                w.u8(b.source_mac_allowlist.len() as u8);
                for mac in &b.source_mac_allowlist {
                    w.bytes(&mac.0);
                }
            }
            Self::SetCsi(b) => w.u8(u8::from(b.enable)),
            Self::SyncPing(b) => w.u64(b.t1_us),
            Self::SyncEcho(b) => {
                w.u64(b.t1_us);
                w.u64(b.t2_us);
                w.u64(b.t3_us);
            }
            Self::Event(b) => {
                w.u8(b.code);
                w.string_u8(&b.detail)?;
            }
        }
        Ok(())
    }

    /// Decode a body of exactly `body` bytes for `opcode`.
    fn decode_from(opcode: Opcode, body: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(body);
        let decoded = match opcode {
            Opcode::JoinRequest => Self::JoinRequest(JoinRequest {
                node: MacAddr(r.array::<6>()?),
                boot: r.u32()?,
                pairing_code: r.u32()?,
                fw_version: r.string_u8()?,
            }),
            Opcode::JoinAck => Self::JoinAck(JoinAck {
                node: MacAddr(r.array::<6>()?),
                token: r.array::<32>()?,
                config_version: r.u32()?,
                config_hash: r.array::<32>()?,
            }),
            Opcode::Result => Self::Result(Ack {
                status: AckStatus::from_u8(r.u8()?),
                detail: r.string_u8()?,
            }),
            Opcode::Status => Self::Status(Status {
                uptime_ms: r.u64()?,
                boot: r.u32()?,
                last_sequence: r.u64()?,
                dropped: r.u32()?,
                ring_depth: r.u16()?,
                temperature_c_x100: r.i16()?,
                config_version: r.u32()?,
                last_rtt_us: r.u32()?,
            }),
            Opcode::GetConfig => Self::GetConfig,
            Opcode::SetConfig => {
                let config_version = r.u32()?;
                let config_hash = r.array::<32>()?;
                let channel = r.u8()?;
                let bandwidth_mhz = r.u8()?;
                let secondary = r.u8()?;
                let csi_enable = r.u8()? != 0;
                let lltf_enable = r.u8()? != 0;
                let ht_ltf_enable = r.u8()? != 0;
                let stbc_ht_ltf_enable = r.u8()? != 0;
                let csi_len_cap = r.u16()?;
                let max_tx_rate_hz = r.u32()?;
                let heartbeat_ms = r.u32()?;
                let count = r.u8()? as usize;
                let mut source_mac_allowlist = Vec::with_capacity(count);
                for _ in 0..count {
                    source_mac_allowlist.push(MacAddr(r.array::<6>()?));
                }
                Self::SetConfig(SetConfig {
                    config_version,
                    config_hash,
                    channel,
                    bandwidth_mhz,
                    secondary,
                    csi_enable,
                    lltf_enable,
                    ht_ltf_enable,
                    stbc_ht_ltf_enable,
                    csi_len_cap,
                    max_tx_rate_hz,
                    heartbeat_ms,
                    source_mac_allowlist,
                })
            }
            Opcode::SetCsi => Self::SetCsi(SetCsi {
                enable: r.u8()? != 0,
            }),
            Opcode::SyncPing => Self::SyncPing(SyncPing { t1_us: r.u64()? }),
            Opcode::SyncEcho => Self::SyncEcho(SyncEcho {
                t1_us: r.u64()?,
                t2_us: r.u64()?,
                t3_us: r.u64()?,
            }),
            Opcode::Reboot => Self::Reboot,
            Opcode::Identify => Self::Identify,
            Opcode::FactoryReset => Self::FactoryReset,
            Opcode::Event => Self::Event(EventNotice {
                code: r.u8()?,
                detail: r.string_u8()?,
            }),
        };
        if r.remaining() != 0 {
            return Err(WireError::BodyLengthMismatch {
                opcode: opcode.to_u8(),
                declared: body.len(),
                consumed: r.consumed(),
            });
        }
        Ok(decoded)
    }
}

/// `JOIN_REQUEST` body: bootstrap pairing (`capture-and-firmware.md` §6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinRequest {
    /// The node's factory base MAC.
    pub node: MacAddr,
    /// The node's boot epoch.
    pub boot: u32,
    /// The short-lived pairing code entered by the operator.
    pub pairing_code: u32,
    /// The node firmware version string.
    pub fw_version: String,
}

/// `JOIN_ACK` body: pairing accepted, with the long-lived token and config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinAck {
    /// The node's factory base MAC.
    pub node: MacAddr,
    /// The long-lived pairing token the node stores in NVS.
    pub token: [u8; 32],
    /// The config version carried alongside.
    pub config_version: u32,
    /// The sha256 of the canonical config.
    pub config_hash: [u8; 32],
}

/// `RESULT` body: a reply status plus a human-readable detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ack {
    /// Whether the request was applied.
    pub status: AckStatus,
    /// A human-readable reason, empty on success.
    pub detail: String,
}

/// `STATUS` body: the node's periodic heartbeat (`capture-and-firmware.md` §6.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// Node uptime in milliseconds.
    pub uptime_ms: u64,
    /// The node's boot epoch.
    pub boot: u32,
    /// The last CSI `sequence` assigned.
    pub last_sequence: u64,
    /// Frames dropped because the ring was full.
    pub dropped: u32,
    /// The current ring depth.
    pub ring_depth: u16,
    /// Temperature in degrees Celsius, scaled by 100.
    pub temperature_c_x100: i16,
    /// The active config version.
    pub config_version: u32,
    /// The last measured core round-trip time in microseconds.
    pub last_rtt_us: u32,
}

/// `SET_CONFIG` body: a transactional config application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetConfig {
    /// The config version being applied.
    pub config_version: u32,
    /// The sha256 of the canonical config, for validation.
    pub config_hash: [u8; 32],
    /// The 802.11 channel.
    pub channel: u8,
    /// The channel width in MHz.
    pub bandwidth_mhz: u8,
    /// The secondary-channel selector.
    pub secondary: u8,
    /// Whether CSI streaming is enabled.
    pub csi_enable: bool,
    /// Whether the legacy LTF is enabled.
    pub lltf_enable: bool,
    /// Whether the HT-LTF is enabled.
    pub ht_ltf_enable: bool,
    /// Whether the STBC-HT-LTF is enabled.
    pub stbc_ht_ltf_enable: bool,
    /// The CSI length cap in bytes.
    pub csi_len_cap: u16,
    /// The maximum transmit rate in hertz.
    pub max_tx_rate_hz: u32,
    /// The heartbeat period in milliseconds.
    pub heartbeat_ms: u32,
    /// An optional allow-list of source MACs.
    pub source_mac_allowlist: Vec<MacAddr>,
}

/// `SET_CSI` body: enable or disable CSI streaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetCsi {
    /// Whether streaming is enabled.
    pub enable: bool,
}

/// `SYNC_PING` body: `t1`, the host send time in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncPing {
    /// Host send time in microseconds.
    pub t1_us: u64,
}

/// `SYNC_ECHO` body: the `t1` echo and the node's receive/send times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncEcho {
    /// The echoed host send time in microseconds.
    pub t1_us: u64,
    /// The node receive time in microseconds.
    pub t2_us: u64,
    /// The node send time in microseconds.
    pub t3_us: u64,
}

/// `EVENT` body: an unsolicited noteworthy event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventNotice {
    /// A node-defined event code.
    pub code: u8,
    /// A human-readable detail.
    pub detail: String,
}

/// Encode one control message as an `AQCC` datagram.
///
/// Fails only when a body cannot be represented: a control string longer than
/// 255 bytes, a config allow-list longer than 255 MACs, or a body longer than
/// 65535 bytes.
pub fn encode_control(message: &ControlMessage) -> Result<Vec<u8>, WireError> {
    let opcode = message.body.opcode();
    let mut body = Writer::new();
    message.body.encode_into(&mut body)?;
    let body = body.into_vec();
    if body.len() > u16::MAX as usize {
        return Err(WireError::BodyTooLong {
            len: body.len(),
            max: u16::MAX as usize,
        });
    }

    let mut w = Writer::new();
    w.bytes(&CONTROL_MAGIC);
    w.u8(CONTROL_VERSION);
    w.u8(opcode.to_u8());
    w.u16(body.len() as u16);
    w.u32(message.request_id);
    w.bytes(&body);
    Ok(w.into_vec())
}

/// Decode one `AQCC` datagram into a [`ControlMessage`].
///
/// Validates the magic, the control version and the body framing before
/// trusting any field. Never panics.
pub fn decode_control(bytes: &[u8]) -> Result<ControlMessage, WireError> {
    let mut r = Reader::new(bytes);

    let magic = r.array::<4>()?;
    if magic != CONTROL_MAGIC {
        return Err(WireError::BadMagic {
            expected: CONTROL_MAGIC,
            found: magic,
        });
    }
    let version = r.u8()?;
    if version != CONTROL_VERSION {
        return Err(WireError::UnsupportedVersion {
            major: version,
            expected: CONTROL_VERSION,
        });
    }
    let opcode_byte = r.u8()?;
    let opcode = Opcode::from_u8(opcode_byte).ok_or(WireError::UnknownOpcode(opcode_byte))?;
    let body_len = r.u16()? as usize;
    let request_id = r.u32()?;

    let total = CONTROL_HEADER_LEN + body_len;
    if bytes.len() < total {
        return Err(WireError::Truncated {
            needed: total,
            actual: bytes.len(),
        });
    }
    if bytes.len() > total {
        return Err(WireError::UnexpectedTrailingBytes {
            extra: bytes.len() - total,
        });
    }

    let body = &bytes[CONTROL_HEADER_LEN..total];
    let body = ControlBody::decode_from(opcode, body)?;
    Ok(ControlMessage { request_id, body })
}

/// The node identity a control message is keyed by, when it names one.
///
/// `JOIN_REQUEST` and `JOIN_ACK` carry the factory MAC directly; other opcodes
/// are matched by `request_id`, so this returns `None` for them.
#[must_use]
pub fn message_mac(message: &ControlMessage) -> Option<MacAddr> {
    match &message.body {
        ControlBody::JoinRequest(b) => Some(b.node),
        ControlBody::JoinAck(b) => Some(b.node),
        _ => None,
    }
}

/// A convenience constructor: a `RESULT` reply echoing `request_id`.
#[must_use]
pub fn result_message(
    request_id: u32,
    status: AckStatus,
    detail: impl Into<String>,
) -> ControlMessage {
    ControlMessage {
        request_id,
        body: ControlBody::Result(Ack {
            status,
            detail: detail.into(),
        }),
    }
}
