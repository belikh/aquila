//! A `Session` and its manifest: the recorded, replayable unit of development.
//!
//! A session is a recorded, replayable, time-bounded stream of CSI frames from
//! one or more nodes, with metadata and provenance. It drives development and
//! scoring ahead of live deployment; it is not the add-on's uptime. Same
//! inputs, same seed, same session implies the same metrics.

use serde::{Deserialize, Serialize};

use crate::error::{require_non_empty, ValidationError};
use crate::ids::{HomeId, NodeId, SessionId};
use crate::version::ContractVersion;

/// A build identity: the commit the run was taken at, and whether the working
/// tree was dirty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildId {
    /// The commit SHA.
    pub git_sha: String,
    /// Whether the working tree had uncommitted changes.
    pub dirty: bool,
}

impl BuildId {
    /// Validate that the commit field is non-empty.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_empty("BuildId.git_sha", &self.git_sha)
    }
}

/// One file in a session archive, with its digest and frame count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionArtifact {
    /// The path within the session archive.
    pub path: String,
    /// SHA-256 digest of the file.
    pub sha256: [u8; 32],
    /// The number of frames the file carries.
    pub frames: u64,
}

impl SessionArtifact {
    /// Validate that the path is non-empty.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_empty("SessionArtifact.path", &self.path)
    }
}

/// The provenance kind of a ground-truth source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GtKind {
    /// A camera-based research reference.
    CameraResearch,
    /// A dedicated reference sensor.
    ReferenceSensor,
    /// Manual labelling.
    Manual,
    /// Synthesised ground truth.
    Synthetic,
}

/// A reference to a ground-truth artefact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroundTruthRef {
    /// The path of the ground-truth artefact.
    pub path: String,
    /// SHA-256 digest of the artefact, or `None` when the digest is not known.
    /// An absent digest is `None`, never an all-zero hash that reads as real
    /// (`sister-issues-core.md` S14).
    pub sha256: Option<[u8; 32]>,
    /// The provenance kind.
    pub kind: GtKind,
}

impl GroundTruthRef {
    /// Validate that the path is non-empty and any digest is non-zero.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_empty("GroundTruthRef.path", &self.path)?;
        if self.sha256 == Some([0u8; 32]) {
            return Err(ValidationError::Invariant {
                detail: "GroundTruthRef.sha256 must not be the all-zero hash".to_string(),
            });
        }
        Ok(())
    }
}

/// A recorded, replayable, time-bounded stream of CSI frames.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// Identity of the session.
    pub id: SessionId,
    /// Human-readable name.
    pub name: String,
    /// The home the session was recorded in.
    pub home: HomeId,
    /// The nodes that contributed frames; non-empty.
    pub nodes: Vec<NodeId>,
    /// Unix wall time the session started, in milliseconds.
    pub started_unix_ms: i64,
    /// Unix wall time the session ended, in milliseconds, when known.
    pub ended_unix_ms: Option<i64>,
    /// The contract version the session was recorded under.
    pub contract: ContractVersion,
    /// The build that produced the session.
    pub build: BuildId,
    /// SHA-256 of the canonical configuration, or `None` when it is not known.
    /// An unknown configuration is `None`, never an all-zero hash presented as a
    /// content address (`sister-issues-core.md` S14).
    pub config_hash: Option<[u8; 32]>,
    /// The RNG seed for any stochastic step.
    pub seed: u64,
    /// The session's artefacts.
    pub artifacts: Vec<SessionArtifact>,
    /// Optional ground-truth reference; research-only.
    pub ground_truth: Option<GroundTruthRef>,
    /// Free-text notes.
    pub notes: String,
}

impl Session {
    /// Validate identity, node set, time ordering and artefacts.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.id.validate()?;
        require_non_empty("Session.name", &self.name)?;
        self.home.validate()?;
        self.build.validate()?;
        if self.nodes.is_empty() {
            return Err(ValidationError::Invariant {
                detail: "Session.nodes must be non-empty".to_string(),
            });
        }
        for node in &self.nodes {
            node.validate_provisioned()?;
        }
        if let Some(ended) = self.ended_unix_ms {
            if ended < self.started_unix_ms {
                return Err(ValidationError::Range {
                    field: "Session.ended_unix_ms",
                    detail: "ended before started".to_string(),
                });
            }
        }
        for artifact in &self.artifacts {
            artifact.validate()?;
        }
        if self.config_hash == Some([0u8; 32]) {
            return Err(ValidationError::Invariant {
                detail: "Session.config_hash must not be the all-zero hash".to_string(),
            });
        }
        if let Some(gt) = &self.ground_truth {
            gt.validate()?;
        }
        Ok(())
    }
}

/// Alias of [`Session`], named for its role as a recorded-fixture metadata
/// record in the interface register.
pub type SessionMeta = Session;

/// A research-only ground-truth record; never a runtime input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundTruthRecord {
    /// The session the record belongs to.
    pub session: SessionId,
    /// Unix wall time, in milliseconds.
    pub t_unix_ms: i64,
    /// The provenance kind.
    pub kind: GtKind,
    /// The scope.
    pub scope: crate::detection::EventScope,
    /// The payload, reusing the event payload vocabulary.
    pub payload: crate::detection::EventPayload,
}

impl GroundTruthRecord {
    /// Validate the session reference and payload.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.session.validate()?;
        self.scope.validate()?;
        self.payload.validate()
    }
}

/// The manifest of a session archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionManifest {
    /// The contract version.
    pub contract: ContractVersion,
    /// The codec label, for example `"postcard-1"`.
    pub codec: String,
    /// The total frame count.
    pub frames: u64,
    /// The files in the archive.
    pub files: Vec<SessionArtifact>,
}

impl SessionManifest {
    /// Validate the codec label and files.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_empty("SessionManifest.codec", &self.codec)?;
        for file in &self.files {
            file.validate()?;
        }
        Ok(())
    }
}
