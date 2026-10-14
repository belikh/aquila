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
    Opcode, CONTROL_AUTH_LEN, CONTROL_HEADER_LEN, CONTROL_MAGIC, CONTROL_VERSION, MAX_SOURCE_MACS,
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
    /// `OTA_BEGIN` (`0x0E`): start a firmware transfer, core → node.
    OtaBegin(OtaBegin),
    /// `OTA_CHUNK` (`0x0F`): one image chunk, core → node.
    OtaChunk(OtaChunk),
    /// `OTA_ACK` (`0x10`): cumulative chunk acknowledgement, node → core.
    OtaAck(OtaAck),
    /// `OTA_STATUS` (`0x11`): OTA progress/state, node → core.
    OtaStatus(OtaStatus),
    /// `OTA_END` (`0x12`): finalise, verify and activate, core → node.
    OtaEnd(OtaEnd),
    /// `OTA_ABORT` (`0x13`): discard the transfer, both directions.
    OtaAbort(OtaAbort),
    /// `OTA_RESULT` (`0x14`): OTA outcome, node → core.
    OtaResult(OtaResult),
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
            Self::OtaBegin(_) => Opcode::OtaBegin,
            Self::OtaChunk(_) => Opcode::OtaChunk,
            Self::OtaAck(_) => Opcode::OtaAck,
            Self::OtaStatus(_) => Opcode::OtaStatus,
            Self::OtaEnd(_) => Opcode::OtaEnd,
            Self::OtaAbort(_) => Opcode::OtaAbort,
            Self::OtaResult(_) => Opcode::OtaResult,
        }
    }

    /// Encode this body, without its header.
    fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        match self {
            Self::JoinRequest(b) => {
                w.bytes(&b.node.0);
                w.u16(b.boot);
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
                w.u16(b.boot);
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
            Self::OtaBegin(b) => {
                w.u32(b.image_id);
                w.u32(b.size);
                w.bytes(&b.sha256);
                w.bytes(&b.signature);
                w.string_u8(&b.version)?;
                w.u16(b.chunk_size);
                w.u8(b.window);
            }
            Self::OtaChunk(b) => {
                w.u32(b.offset);
                w.u16(b.len);
                w.u32(b.crc32);
                w.bytes(&b.bytes);
            }
            Self::OtaAck(b) => {
                w.u32(b.received_upto);
                w.u8(b.status.to_u8());
            }
            Self::OtaStatus(b) => {
                w.u8(b.state.to_u8());
                w.u32(b.received_upto);
                w.u32(b.size);
            }
            Self::OtaEnd(b) => {
                w.u32(b.image_id);
                w.bytes(&b.sha256);
            }
            Self::OtaAbort(b) => {
                w.string_u8(&b.reason)?;
            }
            Self::OtaResult(b) => {
                w.u8(b.outcome.to_u8());
                w.string_u8(&b.reason)?;
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
                boot: r.u16()?,
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
                boot: r.u16()?,
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
            Opcode::OtaBegin => {
                let image_id = r.u32()?;
                let size = r.u32()?;
                let sha256 = r.array::<32>()?;
                let signature = r.array::<64>()?;
                let version = r.string_u8()?;
                let chunk_size = r.u16()?;
                let window = r.u8()?;
                Self::OtaBegin(OtaBegin {
                    image_id,
                    size,
                    sha256,
                    signature,
                    version,
                    chunk_size,
                    window,
                })
            }
            Opcode::OtaChunk => {
                let offset = r.u32()?;
                let len = r.u16()?;
                let crc32 = r.u32()?;
                let bytes = r.take(len as usize)?.to_vec();
                Self::OtaChunk(OtaChunk {
                    offset,
                    len,
                    crc32,
                    bytes,
                })
            }
            Opcode::OtaAck => Self::OtaAck(OtaAck {
                received_upto: r.u32()?,
                status: OtaAckStatus::from_u8(r.u8()?),
            }),
            Opcode::OtaStatus => Self::OtaStatus(OtaStatus {
                state: OtaState::from_u8(r.u8()?),
                received_upto: r.u32()?,
                size: r.u32()?,
            }),
            Opcode::OtaEnd => Self::OtaEnd(OtaEnd {
                image_id: r.u32()?,
                sha256: r.array::<32>()?,
            }),
            Opcode::OtaAbort => Self::OtaAbort(OtaAbort {
                reason: r.string_u8()?,
            }),
            Opcode::OtaResult => Self::OtaResult(OtaResult {
                outcome: OtaOutcome::from_u8(r.u8()?),
                reason: r.string_u8()?,
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
    /// The node's boot epoch (`u16`, matching the CSI contract frame).
    pub boot: u16,
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
    /// The node's boot epoch (`u16`, matching the CSI contract frame).
    pub boot: u16,
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

/// `OTA_BEGIN` body: the image identity and the transfer parameters
/// (`firmware-ota.md` §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaBegin {
    /// A core-chosen transfer id, echoed by `OTA_END`.
    pub image_id: u32,
    /// The whole image size in bytes.
    pub size: u32,
    /// SHA-256 of the whole image.
    pub sha256: [u8; 32],
    /// Ed25519 signature over the SHA-256 digest.
    pub signature: [u8; 64],
    /// The human-readable firmware version.
    pub version: String,
    /// The chunk payload size in bytes.
    pub chunk_size: u16,
    /// The sliding-window depth in chunks.
    pub window: u8,
}

/// `OTA_CHUNK` body: one image chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaChunk {
    /// The byte offset of this chunk within the image.
    pub offset: u32,
    /// The declared payload length; always equal to `bytes.len()`.
    pub len: u16,
    /// CRC-32 (IEEE) of `bytes`.
    pub crc32: u32,
    /// The chunk payload.
    pub bytes: Vec<u8>,
}

/// The status an `OTA_ACK` carries. Unknown bytes are preserved, never guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OtaAckStatus {
    /// The cumulative offset advanced; more chunks wanted.
    Ok = 0,
    /// A chunk failed its CRC; retransmit from `received_upto`.
    Nack = 1,
    /// An unrecognised status.
    Unknown = 255,
}

impl OtaAckStatus {
    /// The on-wire status byte.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Map an on-wire status byte; unknown values become [`OtaAckStatus::Unknown`].
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Ok,
            1 => Self::Nack,
            _ => Self::Unknown,
        }
    }
}

/// `OTA_ACK` body: a cumulative acknowledgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OtaAck {
    /// Every byte below this offset is received; resend from here.
    pub received_upto: u32,
    /// The acknowledgement status.
    pub status: OtaAckStatus,
}

/// The state an `OTA_STATUS` reports. Unknown bytes are preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OtaState {
    /// No transfer in progress.
    Idle = 0,
    /// Receiving chunks.
    Receiving = 1,
    /// Verifying the complete image.
    Verifying = 2,
    /// Setting the boot partition and rebooting.
    Activating = 3,
    /// Complete and healthy.
    Done = 4,
    /// The transfer failed.
    Failed = 5,
    /// Rolled back to the previous image.
    Rollback = 6,
    /// An unrecognised state.
    Unknown = 255,
}

impl OtaState {
    /// The on-wire state byte.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Map an on-wire state byte; unknown values become [`OtaState::Unknown`].
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Idle,
            1 => Self::Receiving,
            2 => Self::Verifying,
            3 => Self::Activating,
            4 => Self::Done,
            5 => Self::Failed,
            6 => Self::Rollback,
            _ => Self::Unknown,
        }
    }
}

/// `OTA_STATUS` body: transfer progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OtaStatus {
    /// The node's OTA state.
    pub state: OtaState,
    /// Every byte below this offset is received.
    pub received_upto: u32,
    /// The whole image size in bytes.
    pub size: u32,
}

/// `OTA_END` body: finalise the transfer and activate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OtaEnd {
    /// The transfer id from `OTA_BEGIN`.
    pub image_id: u32,
    /// SHA-256 of the whole image, for a final check.
    pub sha256: [u8; 32],
}

/// `OTA_ABORT` body: discard the transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaAbort {
    /// A human-readable reason.
    pub reason: String,
}

/// The outcome an `OTA_RESULT` reports. Unknown bytes are preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OtaOutcome {
    /// The image verified, but is not yet activated.
    Verified = 0,
    /// The image is activated and healthy.
    Activated = 1,
    /// The node rolled back to the previous image.
    Rollback = 2,
    /// The transfer or verification failed.
    Failed = 3,
    /// An unrecognised outcome.
    Unknown = 255,
}

impl OtaOutcome {
    /// The on-wire outcome byte.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Map an on-wire outcome byte; unknown values become [`OtaOutcome::Unknown`].
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Verified,
            1 => Self::Activated,
            2 => Self::Rollback,
            3 => Self::Failed,
            _ => Self::Unknown,
        }
    }
}

/// `OTA_RESULT` body: the transfer outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaResult {
    /// The outcome.
    pub outcome: OtaOutcome,
    /// A human-readable reason, empty on success.
    pub reason: String,
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

/// Encode one control message with its per-node authentication trailer.
///
/// The body is encoded exactly as [`encode_control`], then a 32-byte
/// HMAC-SHA256 tag over the header and body (keyed by `token`) is appended. A
/// node that has completed pairing verifies this tag before decoding
/// (`capture-and-firmware.md` §6.3).
pub fn encode_control_authenticated(
    message: &ControlMessage,
    token: &[u8; CONTROL_AUTH_LEN],
) -> Result<Vec<u8>, WireError> {
    let mut bytes = encode_control(message)?;
    let tag = crate::auth::control_auth_tag(token, &bytes);
    bytes.extend_from_slice(&tag);
    Ok(bytes)
}

/// Decode one authenticated `AQCC` datagram.
///
/// The final [`CONTROL_AUTH_LEN`] bytes are treated as the authentication
/// trailer; the tag is verified in constant time against `token` before the
/// header and body are decoded. A message without a trailer, with a tag of the
/// wrong length, or with a tag that does not verify is rejected with
/// [`WireError::AuthenticationFailed`] and never decoded.
pub fn decode_control_authenticated(
    bytes: &[u8],
    token: &[u8; CONTROL_AUTH_LEN],
) -> Result<ControlMessage, WireError> {
    if bytes.len() < CONTROL_AUTH_LEN {
        return Err(WireError::Truncated {
            needed: CONTROL_AUTH_LEN,
            actual: bytes.len(),
        });
    }
    let split = bytes.len() - CONTROL_AUTH_LEN;
    let (frame, tag) = bytes.split_at(split);
    if !crate::auth::verify_control_auth(token, frame, tag) {
        return Err(WireError::AuthenticationFailed);
    }
    decode_control(frame)
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
