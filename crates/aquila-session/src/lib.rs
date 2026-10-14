#![forbid(unsafe_code)]

//! `aquila-session` — the `.aqs` recorded-fixture container.
//!
//! This crate is level 2 in the workspace dependency graph
//! (`docs/spec/interface-register.md`) and depends only on
//! [`aquila_types`] and [`aquila_wire`]. It owns one deep module: the on-disk
//! container for a Session (`docs/spec/capture-and-firmware.md` §8), together
//! with its typed manifest and digest trailer.
//!
//! A Session is a recorded, replayable, time-bounded stream of CSI frames from
//! one or more nodes, with metadata and provenance (`CONTEXT.md`). The container
//! stores the **exact on-wire `AQCI` datagram bytes** plus a typed manifest and
//! a digest trailer, so replay exercises the real [`aquila_wire`] decoder and
//! the wire codec cannot drift from live.
//!
//! # Layout
//!
//! ```text
//! b"AQS" | version u8 | metadata_len u32 (LE) | metadata (canonical JSON)
//! record*   (u8 type | u32 body_len LE | body)
//! b"AQE" | record_count u64 (LE) | sha256(record stream)
//! ```
//!
//! Records: `0x01` CSI frame (`host_arrival_us` + exact `AQCI` datagram),
//! `0x02` ground truth (`host_arrival_us` + JSON), `0x03` event
//! (`host_arrival_us` + JSON), `0x04` annotation (JSON).
//!
//! # Example
//!
//! ```
//! use aquila_session::{verify_bytes, Manifest, Record, SessionReader, SessionWriter};
//! # use aquila_types::{BuildId, ContractVersion, NodeId, Session, Slug};
//! let session = Session {
//!     id: Slug("demo".into()),
//!     name: "demo".into(),
//!     home: Slug("home".into()),
//!     nodes: vec![NodeId(1)],
//!     started_unix_ms: 0,
//!     ended_unix_ms: None,
//!     contract: ContractVersion::new(1, 0),
//!     build: BuildId { git_sha: "deadbeef".into(), dirty: false },
//!     config_hash: None,
//!     seed: 1,
//!     artifacts: vec![],
//!     ground_truth: None,
//!     notes: String::new(),
//! };
//! let manifest = Manifest::new(session);
//! let mut writer = SessionWriter::new(&manifest).unwrap();
//! writer.write_record(&Record::Annotation { json: "{}".into() }).unwrap();
//! let bytes = writer.finish().unwrap();
//! let reader = SessionReader::from_bytes(&bytes).unwrap();
//! assert_eq!(reader.len(), 1);
//! verify_bytes(&bytes).unwrap();
//! ```
//!
//! Australian English throughout (serialisation, behaviour, normalise).

mod container;
mod error;
mod manifest;
mod record;

#[cfg(test)]
mod tests;

pub use container::{
    verify, verify_bytes, SessionDigest, SessionReader, SessionWriter, VerifyReport,
    CONTAINER_VERSION, HEADER_FIXED_LEN, MAGIC_HEADER, MAGIC_TRAILER, RECORD_PREFIX_LEN,
    TRAILER_LEN,
};
pub use error::SessionError;
pub use manifest::{parse_manifest, Elicitation, Manifest, SCHEMA};
pub use record::{Record, RecordKind};
