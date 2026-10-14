//! Identity newtypes: the shared alphabet for naming nodes and places.
//!
//! Two kinds of identity exist. [`NodeId`] is the dense on-air identity unique
//! within one home. Every other entity is named by a [`Slug`]: a stable,
//! human-authorable string such as `"kitchen-01"` or `"ground-floor"`. The
//! `*Id` names ([`HomeId`], [`FloorId`], [`RoomId`], [`ZoneId`], [`PairId`],
//! [`SessionId`], [`MetricId`]) are transparent aliases of [`Slug`] so the
//! vocabulary reads correctly without forking the type.

use serde::{Deserialize, Serialize};

use crate::error::{require_non_empty, ValidationError};

/// A stable, human-authorable identifier for config and topology entities.
///
/// Slugs are lower-case and hyphenated by convention (`"kitchen-01"`), but the
/// type only enforces non-emptiness; the convention is a documentation rule,
/// not a shape rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Slug(pub String);

impl Slug {
    /// Construct a slug, rejecting the empty string.
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        require_non_empty("slug", &value)?;
        Ok(Self(value))
    }

    /// Borrow the slug as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the slug is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Validate the slug's non-emptiness invariant.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_empty("slug", &self.0)
    }
}

/// Identity of a home: the unit of installation and of global calibration.
pub type HomeId = Slug;

/// Identity of a floor: a storey of a home, containing rooms.
pub type FloorId = Slug;

/// Identity of a room: a named physical partition of a floor, and the v1
/// spatial scope of a sensing result.
pub type RoomId = Slug;

/// Identity of a zone: a named spatial region a sensing result is attributed
/// to. In v1 a zone *is* a room; `ZoneId == RoomId`.
pub type ZoneId = RoomId;

/// Identity of a node-pair: the ordered tx/rx edge of the sensing graph.
pub type PairId = Slug;

/// Alias of [`PairId`], matching the `NodePair` record name.
pub type NodePairId = PairId;

/// Identity of a session: one recorded, replayable stream of CSI frames.
pub type SessionId = Slug;

/// Identity of a metric in the commit-keyed ledger.
pub type MetricId = Slug;

/// Dense on-air identity, unique within one home.
///
/// Provisioned via the node control channel at commissioning and mirrored in
/// the topology. Values `1..=65535` are provisioned; `0` is reserved for
/// "unprovisioned" ([`NodeId::UNPROVISIONED`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u16);

impl NodeId {
    /// The reserved "unprovisioned" identity. Never appears in a committed
    /// topology.
    pub const UNPROVISIONED: NodeId = NodeId(0);

    /// Construct a node identity from its dense value.
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Whether this identity names a provisioned node (that is, is non-zero).
    #[must_use]
    pub const fn is_provisioned(&self) -> bool {
        self.0 != NodeId::UNPROVISIONED.0
    }

    /// Validate that the node is provisioned.
    pub fn validate_provisioned(&self) -> Result<(), ValidationError> {
        if self.is_provisioned() {
            Ok(())
        } else {
            Err(ValidationError::Invariant {
                detail: "node id is UNPROVISIONED".to_string(),
            })
        }
    }
}

/// A 48-bit hardware address, carried lower-case on the wire and on disk.
///
/// Per ADR-0003 §3 the factory MAC is the on-air identity at the wire seam;
/// [`NodeId`] is the canonical *contract* identity, and `aquila-wire` maps
/// between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MacAddr(pub [u8; 6]);

impl MacAddr {
    /// Construct a hardware address from its six octets.
    #[must_use]
    pub const fn new(octets: [u8; 6]) -> Self {
        Self(octets)
    }

    /// The six octets, big-endian (most-significant first).
    #[must_use]
    pub const fn octets(&self) -> [u8; 6] {
        self.0
    }
}

/// Session-scoped anonymous subject index.
///
/// Stable only within a session. It is not person identity, and is never
/// persisted across sessions as identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SubjectId(pub u32);

impl SubjectId {
    /// Construct a subject index.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}
