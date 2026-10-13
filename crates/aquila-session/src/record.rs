//! The typed record stream of a `.aqs` container.
//!
//! Each record is `u8 type | u32 body_len | body` (`capture-and-firmware.md`
//! §8.2). A CSI record stores the **exact on-wire `AQCI` datagram bytes**, so
//! replay always runs the real `aquila-wire` decoder and the wire codec cannot
//! drift from live. Ground-truth, event and annotation records carry JSON.
//!
//! Ground-truth records are research-only and are never consumed at runtime
//! (`capture-and-firmware.md` §8.5); the container still stores them so an
//! evaluation session is self-labelling.

use crate::error::SessionError;

/// The on-wire type byte of a session record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RecordKind {
    /// Type `0x01`: a CSI frame, carrying an exact `AQCI` datagram.
    Csi = 0x01,
    /// Type `0x02`: a research-only ground-truth record, JSON body.
    GroundTruth = 0x02,
    /// Type `0x03`: a session event, JSON body.
    Event = 0x03,
    /// Type `0x04`: a free-form operator annotation, JSON body.
    Annotation = 0x04,
}

impl RecordKind {
    /// The on-wire type byte.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Parse a type byte; `None` for an unknown type.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::Csi),
            0x02 => Some(Self::GroundTruth),
            0x03 => Some(Self::Event),
            0x04 => Some(Self::Annotation),
            _ => None,
        }
    }

    /// A stable label for error messages and tests.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Csi => "csi",
            Self::GroundTruth => "ground_truth",
            Self::Event => "event",
            Self::Annotation => "annotation",
        }
    }
}

/// One typed record in a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record {
    /// A CSI frame: the host arrival time in microseconds and the exact on-wire
    /// `AQCI` datagram bytes.
    Csi {
        /// Host arrival time in microseconds.
        host_arrival_us: u64,
        /// The exact `AQCI` datagram bytes, decoded on replay by `aquila-wire`.
        datagram: Vec<u8>,
    },
    /// A ground-truth record: host arrival time in microseconds and a JSON body.
    GroundTruth {
        /// Host arrival time in microseconds.
        host_arrival_us: u64,
        /// The JSON ground-truth body.
        json: String,
    },
    /// A session event: host arrival time in microseconds and a JSON body.
    Event {
        /// Host arrival time in microseconds.
        host_arrival_us: u64,
        /// The JSON event body.
        json: String,
    },
    /// A free-form operator annotation, JSON body only.
    Annotation {
        /// The JSON annotation body.
        json: String,
    },
}

impl Record {
    /// The record's type.
    #[must_use]
    pub const fn kind(&self) -> RecordKind {
        match self {
            Self::Csi { .. } => RecordKind::Csi,
            Self::GroundTruth { .. } => RecordKind::GroundTruth,
            Self::Event { .. } => RecordKind::Event,
            Self::Annotation { .. } => RecordKind::Annotation,
        }
    }

    /// The host arrival time in microseconds, when the record carries one.
    #[must_use]
    pub const fn host_arrival_us(&self) -> Option<u64> {
        match self {
            Self::Csi { host_arrival_us, .. }
            | Self::GroundTruth { host_arrival_us, .. }
            | Self::Event { host_arrival_us, .. } => Some(*host_arrival_us),
            Self::Annotation { .. } => None,
        }
    }

    /// Append the encoded record to `out`.
    pub(crate) fn encode_into(&self, out: &mut Vec<u8>) {
        let kind = self.kind();
        let body_len = self.body_len();
        out.push(kind.to_u8());
        out.extend_from_slice(&(body_len as u32).to_le_bytes());
        match self {
            Self::Csi {
                host_arrival_us,
                datagram,
            } => {
                out.extend_from_slice(&host_arrival_us.to_le_bytes());
                out.extend_from_slice(datagram);
            }
            Self::GroundTruth { host_arrival_us, json }
            | Self::Event { host_arrival_us, json } => {
                out.extend_from_slice(&host_arrival_us.to_le_bytes());
                out.extend_from_slice(json.as_bytes());
            }
            Self::Annotation { json } => out.extend_from_slice(json.as_bytes()),
        }
    }

    /// The length of the body, excluding the five-byte type-and-length prefix.
    #[must_use]
    pub(crate) fn body_len(&self) -> usize {
        match self {
            Self::Csi { datagram, .. } => 8 + datagram.len(),
            Self::GroundTruth { json, .. } | Self::Event { json, .. } => 8 + json.len(),
            Self::Annotation { json } => json.len(),
        }
    }

    /// Decode a record body given its already-read type.
    pub(crate) fn decode(kind: RecordKind, body: &[u8]) -> Result<Self, SessionError> {
        match kind {
            RecordKind::Csi => {
                if body.len() < 8 {
                    return Err(SessionError::RecordTooShort {
                        kind: kind.name(),
                        len: body.len(),
                    });
                }
                let host_arrival_us = u64::from_le_bytes(body[..8].try_into().expect("8 bytes"));
                Ok(Self::Csi {
                    host_arrival_us,
                    datagram: body[8..].to_vec(),
                })
            }
            RecordKind::GroundTruth => {
                let (host_arrival_us, json) = split_timed_json(kind, body)?;
                Ok(Self::GroundTruth {
                    host_arrival_us,
                    json,
                })
            }
            RecordKind::Event => {
                let (host_arrival_us, json) = split_timed_json(kind, body)?;
                Ok(Self::Event {
                    host_arrival_us,
                    json,
                })
            }
            RecordKind::Annotation => Ok(Self::Annotation {
                json: utf8(body)?,
            }),
        }
    }
}

/// Split a `host_arrival_us + JSON` body.
fn split_timed_json(kind: RecordKind, body: &[u8]) -> Result<(u64, String), SessionError> {
    if body.len() < 8 {
        return Err(SessionError::RecordTooShort {
            kind: kind.name(),
            len: body.len(),
        });
    }
    let host_arrival_us = u64::from_le_bytes(body[..8].try_into().expect("8 bytes"));
    Ok((host_arrival_us, utf8(&body[8..])?))
}

/// Decode a body as UTF-8, preserving the typed error.
fn utf8(bytes: &[u8]) -> Result<String, SessionError> {
    core::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| SessionError::InvalidUtf8)
}
