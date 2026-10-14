#![forbid(unsafe_code)]

//! `aquila-types` — the shared, versioned vocabulary crate for Aquila.
//!
//! This crate is level 0 in the workspace dependency graph: it depends on no
//! other Aquila crate. It declares the canonical contract types from
//! `docs/spec/data-contracts.md` exactly once, together with the one-method
//! [`Clock`] trait from ADR-0003 §5. Its only behaviour is constructors,
//! invariant validation and the workspace's single [`sha256`] primitive.
//!
//! Vocabulary follows `CONTEXT.md`; use its terms and no others. Every other
//! crate imports these types, and no crate may declare a second, parallel
//! version of them. Where a *shape* is needed that is *almost* one of these,
//! add an optional defaulted field or wrap the type — never fork it.
//!
//! # Versions
//!
//! There are exactly two version numbers and they move together: the contract
//! version ([`CONTRACT_VERSION`], carried by every session, ledger record and
//! archive) and the wire version (owned by `aquila-wire`). A contract is
//! append-only within a major version.
//!
//! # Prose
//!
//! Australian English throughout (serialisation, behaviour, metres,
//! normalise, synchronise).

mod calibration;
mod csi;
mod detection;
mod error;
mod feature;
mod hash;
mod ids;
mod ledger;
mod pose;
mod primitives;
mod session;
mod time;
mod topology;
mod version;
mod vitals;

#[cfg(test)]
mod tests;

pub use calibration::{Baseline, BaselineScope, Calibration, ClockModel, PairCorrection};
pub use csi::{ClockGrid, CsiEnvelope, CsiFrame, SensingFrame, SensingScope};
pub use detection::{
    Detection, DetectionKind, DetectionScope, DetectionValue, Event, EventKind, EventPayload,
    EventScope, MAX_COUNT_V1,
};
pub use error::ValidationError;
pub use feature::FeatureVector;
pub use hash::{sha256, to_hex};
pub use ids::{
    FloorId, HomeId, MacAddr, MetricId, NodeId, NodePairId, PairId, RoomId, SessionId, Slug,
    SubjectId, ZoneId,
};
pub use ledger::{
    DatasetRef, EnvFingerprint, Interval, LedgerRecord, MetricKind, ModelRef, Split,
};
pub use pose::{Keypoint, KeypointId, Pose, KEYPOINT_COUNT};
pub use primitives::{
    Band, Confidence, NodeQuality, PhyFlags, Quality, SecondaryChannel, Witness,
};
pub use session::{
    BuildId, GroundTruthRecord, GroundTruthRef, GtKind, Session, SessionArtifact, SessionManifest,
    SessionMeta,
};
pub use time::{Clock, CoreTime, DeviceTime, TimeSpan, TimeWindow};
pub use topology::{
    DeviceType, FirmwareRef, Floor, Home, Node, NodePair, PairScope, Room, RoomKind, Topology,
};
pub use version::{
    ContractVersion, CONTRACT_VERSION, CONTRACT_VERSION_MAJOR, CONTRACT_VERSION_MINOR,
};
pub use vitals::{Estimate, VitalsReading, VitalsStatus};
