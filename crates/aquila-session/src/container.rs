//! The `.aqs` container: header, typed record stream and digest trailer.
//!
//! Layout (`capture-and-firmware.md` §8.1):
//!
//! ```text
//! b"AQS"        (3)  header magic
//! version       (1)  container schema version, currently 1
//! metadata_len  (4)  little-endian u32 length of the manifest JSON
//! metadata      (*)  canonical manifest JSON
//! record*            typed, length-prefixed records
//! b"AQE"        (3)  trailer magic
//! record_count  (8)  little-endian u64 number of records
//! sha256        (32) digest of the record stream
//! ```
//!
//! Each record is `type (1) | body_len (4, LE) | body`. A CSI record's body is
//! `host_arrival_us (8, LE) | datagram`, where `datagram` is the exact on-wire
//! `AQCI` bytes: replay always runs the real `aquila-wire` decoder.
//!
//! The digest covers exactly the record-stream bytes between the header and the
//! trailer, so [`verify`] can recompute it without trusting any field. The
//! reader parses the whole container eagerly; committed clips are short
//! (`capture-and-firmware.md` §8.4) and an eager parse is the simplest way to
//! locate the fixed-size trailer and report typed failures.

use std::io::{Read, Write};
use std::path::Path;

use crate::error::SessionError;
use crate::manifest::{parse_manifest, Manifest};
use crate::record::{Record, RecordKind};
use aquila_types::{sha256, to_hex as hex};

/// The container schema version written by this crate.
pub const CONTAINER_VERSION: u8 = 1;

/// The header magic, ASCII `AQS`.
pub const MAGIC_HEADER: [u8; 3] = *b"AQS";

/// The trailer magic, ASCII `AQE`.
pub const MAGIC_TRAILER: [u8; 3] = *b"AQE";

/// The fixed header length before the manifest JSON.
pub const HEADER_FIXED_LEN: usize = 8;

/// The fixed trailer length: magic, count and digest.
pub const TRAILER_LEN: usize = 3 + 8 + 32;

/// The length of a record's `type | body_len` prefix.
pub const RECORD_PREFIX_LEN: usize = 5;

/// The digest and record count written to a container's trailer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionDigest {
    /// The number of records in the stream.
    pub record_count: u64,
    /// The SHA-256 of the record stream.
    pub digest: [u8; 32],
}

/// Build a `.aqs` container from a manifest and a sequence of records.
///
/// This is the streaming writer: records accumulate in memory and the fixed
/// header and digest trailer are written by [`finish`](SessionWriter::finish).
/// A synthesised clip is short by design (`capture-and-firmware.md` §8.4), so
/// buffering the record stream costs little and makes the digest exact.
#[derive(Debug, Clone)]
pub struct SessionWriter {
    manifest_json: Vec<u8>,
    record_stream: Vec<u8>,
    count: u64,
}

impl SessionWriter {
    /// Start a writer for `manifest`, validating the manifest up front.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Manifest`] if the manifest is invalid, or
    /// [`SessionError::Json`] if it cannot be serialised.
    pub fn new(manifest: &Manifest) -> Result<Self, SessionError> {
        manifest.validate()?;
        Ok(Self {
            manifest_json: manifest.to_canonical_json()?,
            record_stream: Vec::new(),
            count: 0,
        })
    }

    /// Append one typed record.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::LengthOutOfRange`] if the record's body length
    /// cannot be represented in the four-byte length field.
    pub fn write_record(&mut self, record: &Record) -> Result<(), SessionError> {
        if record.body_len() as u64 > u32::MAX as u64 {
            return Err(SessionError::LengthOutOfRange {
                field: "record body_len",
                value: record.body_len() as u64,
            });
        }
        record.encode_into(&mut self.record_stream);
        self.count += 1;
        Ok(())
    }

    /// Append one CSI record: a host arrival time and exact `AQCI` datagram.
    ///
    /// # Errors
    ///
    /// As [`write_record`](SessionWriter::write_record).
    pub fn write_frame(
        &mut self,
        host_arrival_us: u64,
        datagram: &[u8],
    ) -> Result<(), SessionError> {
        self.write_record(&Record::Csi {
            host_arrival_us,
            datagram: datagram.to_vec(),
        })
    }

    /// Append a JSON ground-truth record.
    ///
    /// # Errors
    ///
    /// As [`write_record`](SessionWriter::write_record).
    pub fn write_ground_truth(
        &mut self,
        host_arrival_us: u64,
        json: impl Into<String>,
    ) -> Result<(), SessionError> {
        self.write_record(&Record::GroundTruth {
            host_arrival_us,
            json: json.into(),
        })
    }

    /// The number of records written so far.
    #[must_use]
    pub const fn record_count(&self) -> u64 {
        self.count
    }

    /// Finish the container and return the complete `.aqs` bytes.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::LengthOutOfRange`] if the manifest JSON is too
    /// large for its four-byte length field.
    pub fn finish(self) -> Result<Vec<u8>, SessionError> {
        let metadata_len = u32::try_from(self.manifest_json.len()).map_err(|_| {
            SessionError::LengthOutOfRange {
                field: "metadata_len",
                value: self.manifest_json.len() as u64,
            }
        })?;

        let mut out = Vec::with_capacity(
            HEADER_FIXED_LEN + self.manifest_json.len() + self.record_stream.len() + TRAILER_LEN,
        );
        out.extend_from_slice(&MAGIC_HEADER);
        out.push(CONTAINER_VERSION);
        out.extend_from_slice(&metadata_len.to_le_bytes());
        out.extend_from_slice(&self.manifest_json);
        out.extend_from_slice(&self.record_stream);
        let digest = sha256(&self.record_stream);
        out.extend_from_slice(&MAGIC_TRAILER);
        out.extend_from_slice(&self.count.to_le_bytes());
        out.extend_from_slice(&digest);
        Ok(out)
    }

    /// Finish the container, writing it to `sink`, and return its digest.
    ///
    /// # Errors
    ///
    /// As [`finish`](SessionWriter::finish), plus [`SessionError::Io`].
    pub fn finish_to<W: Write>(self, mut sink: W) -> Result<SessionDigest, SessionError> {
        let digest = SessionDigest {
            record_count: self.count,
            digest: sha256(&self.record_stream),
        };
        let bytes = self.finish()?;
        sink.write_all(&bytes)?;
        Ok(digest)
    }
}

/// A fully parsed `.aqs` container.
#[derive(Debug, Clone)]
pub struct SessionReader {
    version: u8,
    manifest: Manifest,
    records: Vec<Record>,
    record_stream: Vec<u8>,
    digest: SessionDigest,
}

impl SessionReader {
    /// Read and parse a container from a file path.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Io`] if the file cannot be read, or any parse
    /// error from [`from_bytes`](SessionReader::from_bytes).
    pub fn open(path: &Path) -> Result<Self, SessionError> {
        let mut file = std::fs::File::open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Self::from_bytes(&bytes)
    }

    /// Read and parse a container from any reader.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Io`] if the reader fails, or any parse error from
    /// [`from_bytes`](SessionReader::from_bytes).
    pub fn from_reader<R: Read>(mut reader: R) -> Result<Self, SessionError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Self::from_bytes(&bytes)
    }

    /// Parse a container from an in-memory byte slice.
    ///
    /// Parsing is structural: the manifest, every record and the trailer are
    /// validated, but the digest is only *read*, not recomputed. Call
    /// [`verify`](SessionReader::verify) to check integrity.
    ///
    /// # Errors
    ///
    /// Returns a typed [`SessionError`] for a bad magic, unsupported version,
    /// truncation, unknown record type, malformed record body or invalid
    /// manifest. Never panics.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SessionError> {
        if bytes.len() < HEADER_FIXED_LEN + TRAILER_LEN {
            return Err(SessionError::Truncated {
                needed: HEADER_FIXED_LEN + TRAILER_LEN,
                actual: bytes.len(),
            });
        }

        let magic: [u8; 3] = bytes[..3].try_into().expect("3 bytes");
        if magic != MAGIC_HEADER {
            return Err(SessionError::BadMagic {
                expected: MAGIC_HEADER,
                found: magic,
            });
        }
        let version = bytes[3];
        if version != CONTAINER_VERSION {
            return Err(SessionError::UnsupportedVersion {
                found: version,
                expected: CONTAINER_VERSION,
            });
        }
        let metadata_len = u32::from_le_bytes(bytes[4..8].try_into().expect("4 bytes")) as usize;
        let metadata_end =
            HEADER_FIXED_LEN
                .checked_add(metadata_len)
                .ok_or(SessionError::LengthOutOfRange {
                    field: "metadata_len",
                    value: metadata_len as u64,
                })?;
        let record_end = bytes.len() - TRAILER_LEN;
        if metadata_end > record_end {
            return Err(SessionError::Truncated {
                needed: metadata_end + TRAILER_LEN,
                actual: bytes.len(),
            });
        }

        let manifest = parse_manifest(&bytes[HEADER_FIXED_LEN..metadata_end])?;
        manifest.validate()?;

        let trailer = &bytes[record_end..];
        let trailer_magic: [u8; 3] = trailer[..3].try_into().expect("3 bytes");
        if trailer_magic != MAGIC_TRAILER {
            return Err(SessionError::BadTrailer {
                expected: MAGIC_TRAILER,
                found: trailer_magic,
            });
        }
        let record_count = u64::from_le_bytes(trailer[3..11].try_into().expect("8 bytes"));
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&trailer[11..43]);

        let record_stream = bytes[metadata_end..record_end].to_vec();
        let records = parse_records(&record_stream)?;

        Ok(Self {
            version,
            manifest,
            records,
            record_stream,
            digest: SessionDigest {
                record_count,
                digest,
            },
        })
    }

    /// The container schema version.
    #[must_use]
    pub const fn version(&self) -> u8 {
        self.version
    }

    /// The parsed manifest.
    #[must_use]
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Every parsed record, in stream order.
    #[must_use]
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// The number of parsed records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the container holds no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The digest and count recorded in the trailer (not yet recomputed).
    #[must_use]
    pub const fn trailer_digest(&self) -> SessionDigest {
        self.digest
    }

    /// Iterate the CSI records as `(host_arrival_us, datagram)` pairs.
    ///
    /// Other record kinds are skipped.
    pub fn frames(&self) -> impl Iterator<Item = (u64, &[u8])> {
        self.records.iter().filter_map(|record| match record {
            Record::Csi {
                host_arrival_us,
                datagram,
            } => Some((*host_arrival_us, datagram.as_slice())),
            _ => None,
        })
    }

    /// Recompute the record-stream digest and compare it with the trailer.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::DigestMismatch`] or
    /// [`SessionError::RecordCountMismatch`] if the container was corrupted.
    pub fn verify(&self) -> Result<VerifyReport, SessionError> {
        let found = sha256(&self.record_stream);
        if found != self.digest.digest {
            return Err(SessionError::DigestMismatch {
                expected: hex(&self.digest.digest),
                found: hex(&found),
            });
        }
        let parsed = self.records.len() as u64;
        if parsed != self.digest.record_count {
            return Err(SessionError::RecordCountMismatch {
                expected: self.digest.record_count,
                found: parsed,
            });
        }
        self.manifest.validate()?;
        Ok(VerifyReport {
            version: self.version,
            record_count: parsed,
            frames: self.frames().count() as u64,
            digest: found,
            session_id: self.manifest.session.id.0.clone(),
        })
    }
}

/// The outcome of a successful [`verify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    /// The container schema version.
    pub version: u8,
    /// The number of records.
    pub record_count: u64,
    /// The number of CSI records.
    pub frames: u64,
    /// The recomputed record-stream digest.
    pub digest: [u8; 32],
    /// The session identity from the manifest.
    pub session_id: String,
}

/// Validate a `.aqs` container file against its manifest and digest.
///
/// # Errors
///
/// Any [`SessionError`] from reading, parsing or digest verification.
pub fn verify(path: &Path) -> Result<VerifyReport, SessionError> {
    SessionReader::open(path)?.verify()
}

/// Validate an in-memory `.aqs` container against its manifest and digest.
///
/// # Errors
///
/// Any [`SessionError`] from parsing or digest verification.
pub fn verify_bytes(bytes: &[u8]) -> Result<VerifyReport, SessionError> {
    SessionReader::from_bytes(bytes)?.verify()
}

/// Parse the record stream into typed records.
fn parse_records(stream: &[u8]) -> Result<Vec<Record>, SessionError> {
    let mut records = Vec::new();
    let mut pos = 0usize;
    while pos < stream.len() {
        let remaining = stream.len() - pos;
        if remaining < RECORD_PREFIX_LEN {
            return Err(SessionError::Truncated {
                needed: RECORD_PREFIX_LEN,
                actual: remaining,
            });
        }
        let kind_byte = stream[pos];
        let kind =
            RecordKind::from_u8(kind_byte).ok_or(SessionError::UnknownRecordType(kind_byte))?;
        let body_len =
            u32::from_le_bytes(stream[pos + 1..pos + 5].try_into().expect("4 bytes")) as usize;
        let body_start = pos + RECORD_PREFIX_LEN;
        let body_end = body_start
            .checked_add(body_len)
            .ok_or(SessionError::LengthOutOfRange {
                field: "record body_len",
                value: body_len as u64,
            })?;
        if body_end > stream.len() {
            return Err(SessionError::Truncated {
                needed: body_end,
                actual: stream.len(),
            });
        }
        records.push(Record::decode(kind, &stream[body_start..body_end])?);
        pos = body_end;
    }
    Ok(records)
}
