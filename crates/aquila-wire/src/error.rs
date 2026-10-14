//! The typed error surface for both wire planes.
//!
//! The decoders never panic: every malformed datagram, short buffer, unknown
//! magic, mismatched version or inconsistent length is reported as a
//! [`WireError`]. Callers that tolerate a lossy live stream (`aquila-sources`
//! `UdpLiveSource`) count these rather than ending the stream.

use thiserror::Error;

/// Everything that can go wrong decoding (or, for the control plane, encoding)
/// an Aquila wire datagram.
///
/// The variants are data about a malformed buffer. They carry no behaviour and
/// name the offending quantity so the caller can log or count the failure
/// without re-deriving it.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WireError {
    /// The buffer ended before a complete field (or datagram) could be read.
    #[error("datagram is truncated: need {needed} bytes, have {actual}")]
    Truncated {
        /// The number of bytes required at the point of failure.
        needed: usize,
        /// The number of bytes actually available.
        actual: usize,
    },
    /// The leading four bytes were not the expected ASCII magic.
    #[error("bad magic: expected {expected:?}, found {found:?}")]
    BadMagic {
        /// The magic this codec expects.
        expected: [u8; 4],
        /// The magic that was found.
        found: [u8; 4],
    },
    /// The wire major version is one this codec cannot speak.
    ///
    /// A minor-version mismatch is accepted (the format is append-only within a
    /// major version); only the major component is rejected.
    #[error("unsupported wire major version {major} (this codec speaks {expected})")]
    UnsupportedVersion {
        /// The major version carried by the datagram.
        major: u8,
        /// The major version this codec implements.
        expected: u8,
    },
    /// The CSI message type byte was not the one known type (`1`).
    #[error("unknown CSI message type {0}")]
    UnknownMessageType(u8),
    /// The subcarrier format byte was not the one known format (`0`, `f32`).
    #[error("unknown subcarrier format {0}")]
    UnknownSubcarrierFormat(u8),
    /// The declared payload length disagrees with the header's subcarrier count.
    #[error("payload length {declared} does not match a body of {expected} bytes")]
    LengthMismatch {
        /// The `payload_len` written in the header.
        declared: usize,
        /// The body length implied by `n_subcarriers`.
        expected: usize,
    },
    /// The subcarrier count is zero or exceeds the datagram budget.
    #[error("subcarrier count {count} is out of range 1..={max}")]
    SubcarrierCount {
        /// The declared subcarrier count.
        count: u16,
        /// The largest supported subcarrier count.
        max: u16,
    },
    /// The control opcode byte is not one this codec knows.
    #[error("unknown control opcode {0:#04x}")]
    UnknownOpcode(u8),
    /// The control body's declared length disagrees with the bytes its fields
    /// actually consume.
    #[error(
        "control opcode {opcode:#04x}: declared {declared} body bytes but consumed {consumed}"
    )]
    BodyLengthMismatch {
        /// The opcode whose body failed.
        opcode: u8,
        /// The `body_len` written in the control header.
        declared: usize,
        /// The bytes the body's fields actually consume.
        consumed: usize,
    },
    /// A control datagram carried bytes after the declared body.
    #[error("{extra} unexpected trailing bytes after the control body")]
    UnexpectedTrailingBytes {
        /// The number of surplus bytes.
        extra: usize,
    },
    /// A length-prefixed control string is longer than its one-byte prefix can
    /// describe.
    #[error("control string is too long: {len} bytes, max {max}")]
    StringTooLong {
        /// The offered length.
        len: usize,
        /// The maximum representable length.
        max: usize,
    },
    /// A config allow-list names more source MACs than its one-byte count can
    /// describe.
    #[error("too many source MACs: {count}, max {max}")]
    TooManyMacs {
        /// The offered count.
        count: usize,
        /// The maximum representable count.
        max: usize,
    },
    /// A control body is larger than its two-byte length prefix can describe.
    #[error("control body is too long: {len} bytes, max {max}")]
    BodyTooLong {
        /// The offered body length.
        len: usize,
        /// The maximum representable body length.
        max: usize,
    },
    /// A length-prefixed control field was not valid UTF-8.
    #[error("control string is not valid UTF-8")]
    InvalidUtf8,
    /// A control message's authentication trailer was absent, the wrong length,
    /// or did not verify against the per-node token (`capture-and-firmware.md`
    /// §6.3). The message is rejected before it is decoded.
    #[error("control message authentication failed")]
    AuthenticationFailed,
    /// A node identity mapping was asked to bind a key that is already bound to
    /// a different counterpart.
    #[error("node identity conflict on {kind}")]
    IdentityConflict {
        /// Which half of the mapping conflicted: `"mac"` or `"node id"`.
        kind: &'static str,
    },
}

/// The historical name for the CSI decode error, kept for the `aquila-sources`
/// seam (`capture-and-firmware.md` §3.1 names `aquila_wire::DecodeError`).
pub type DecodeError = WireError;
