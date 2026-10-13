//! The typed error surface for the `.aqs` recorded-fixture container.
//!
//! Reading, writing and verifying a session never panic: every malformed
//! header, truncated record, bad magic, digest mismatch or invalid manifest is
//! reported as a [`SessionError`]. The container is a fixture boundary, so a
//! corrupted session must fail loudly and typed rather than produce a
//! plausible-looking but wrong stream.

use thiserror::Error;

/// Everything that can go wrong reading, writing or verifying a `.aqs` session.
///
/// The variants name the offending quantity so a caller can log or assert on
/// the exact failure without re-deriving it. No variant is a panic.
#[derive(Debug, Error)]
pub enum SessionError {
    /// An underlying I/O operation failed.
    #[error("session I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The manifest or a JSON record body was not valid JSON, or did not match
    /// the expected shape.
    #[error("session manifest is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// The container's leading magic was not `AQS`.
    #[error("bad session magic: expected {expected:?}, found {found:?}")]
    BadMagic {
        /// The magic this container expects.
        expected: [u8; 3],
        /// The magic that was found.
        found: [u8; 3],
    },
    /// The trailing magic was not `AQE`.
    #[error("bad session trailer magic: expected {expected:?}, found {found:?}")]
    BadTrailer {
        /// The trailer magic this container expects.
        expected: [u8; 3],
        /// The trailer magic that was found.
        found: [u8; 3],
    },
    /// The container schema version is one this reader cannot speak.
    #[error("unsupported session container version {found} (this reader speaks {expected})")]
    UnsupportedVersion {
        /// The version byte carried by the container.
        found: u8,
        /// The version this reader implements.
        expected: u8,
    },
    /// The buffer ended before a complete header, record or trailer was read.
    #[error("session container is truncated: need {needed} bytes, have {actual}")]
    Truncated {
        /// The number of bytes required.
        needed: usize,
        /// The number of bytes available.
        actual: usize,
    },
    /// An untrusted length field would overflow a platform `usize` or the
    /// remaining buffer.
    #[error("session length field {field} is out of range: {value}")]
    LengthOutOfRange {
        /// The name of the offending length field.
        field: &'static str,
        /// The untrusted value.
        value: u64,
    },
    /// A record's type byte was not one this container knows.
    #[error("unknown session record type {0:#04x}")]
    UnknownRecordType(u8),
    /// A record body was shorter than its type requires.
    #[error("session record of type {kind:?} is too short: {len} bytes")]
    RecordTooShort {
        /// The record type name.
        kind: &'static str,
        /// The declared body length.
        len: usize,
    },
    /// A JSON record body was not valid UTF-8.
    #[error("session record body is not valid UTF-8")]
    InvalidUtf8,
    /// The record stream digest did not match the trailer.
    #[error("session digest mismatch: trailer {expected}, recomputed {found}")]
    DigestMismatch {
        /// The digest recorded in the trailer, hex-encoded.
        expected: String,
        /// The digest recomputed over the record stream, hex-encoded.
        found: String,
    },
    /// The trailer's record count did not match the parsed record count.
    #[error("session record count mismatch: trailer {expected}, parsed {found}")]
    RecordCountMismatch {
        /// The count recorded in the trailer.
        expected: u64,
        /// The count actually parsed.
        found: u64,
    },
    /// The parsed manifest failed a domain invariant.
    #[error("session manifest is invalid: {0}")]
    Manifest(String),
    /// A digest trailer sha256 was not exactly 32 bytes; only produced on write
    /// if a caller supplies an already-hashed value of the wrong length.
    #[error("session digest must be 32 bytes, got {0}")]
    BadDigestLength(usize),
}

impl SessionError {
    /// A short, stable label for the failure, useful in tests and logs.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Json(_) => "json",
            Self::BadMagic { .. } => "bad_magic",
            Self::BadTrailer { .. } => "bad_trailer",
            Self::UnsupportedVersion { .. } => "unsupported_version",
            Self::Truncated { .. } => "truncated",
            Self::LengthOutOfRange { .. } => "length_out_of_range",
            Self::UnknownRecordType(_) => "unknown_record_type",
            Self::RecordTooShort { .. } => "record_too_short",
            Self::InvalidUtf8 => "invalid_utf8",
            Self::DigestMismatch { .. } => "digest_mismatch",
            Self::RecordCountMismatch { .. } => "record_count_mismatch",
            Self::Manifest(_) => "manifest",
            Self::BadDigestLength(_) => "bad_digest_length",
        }
    }
}
