//! The `GroundTruth` seam: the only source of reference labels for scoring.
//!
//! A tiny interface (`kind`/`label`/`coverage`/`provenance`) hides each
//! adapter's capture format, clock alignment and calibration
//! (`evaluation.md` §2). Ground truth is an **offline** evaluation seam: it is
//! never a runtime input to presence, pose or vitals.
//!
//! [`FixtureGroundTruth`] is the v1 default adapter: it labels a Session from a
//! committed fixture set, or from a scene's synthesised reference stream
//! (`capture-and-firmware.md` §7.3).

use std::collections::BTreeSet;

use aquila_simulator::{SceneScript, SimConfig, SimulatorSource};
use aquila_types::{
    sha256, to_hex, EventPayload, EventScope, GroundTruthRecord, GtKind, RoomId, SessionId,
    TimeWindow,
};

use crate::error::EvaluationError;

/// What a `GroundTruth` source can label (`evaluation.md` §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    /// Per-zone binary presence.
    Presence,
    /// Per-zone people count.
    Count,
    /// Coarse pose.
    Pose,
    /// Breathing rate.
    Breathing,
    /// Heart rate.
    Heart,
}

impl ReferenceKind {
    /// Whether a record payload carries this kind of reference.
    #[must_use]
    pub fn matches(self, payload: &EventPayload) -> bool {
        match (self, payload) {
            (ReferenceKind::Presence, EventPayload::Presence { .. }) => true,
            (ReferenceKind::Count, EventPayload::Count { .. }) => true,
            (ReferenceKind::Pose, EventPayload::Change { .. }) => false,
            (ReferenceKind::Breathing, EventPayload::Vitals { metric, .. }) => {
                metric == "breathing_rate" || metric == "breathing"
            }
            (ReferenceKind::Heart, EventPayload::Vitals { metric, .. }) => {
                metric == "heart_rate" || metric == "heart"
            }
            _ => false,
        }
    }
}

/// One aligned reference record for scoring (`evaluation.md` §2.1).
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceRecord {
    /// The session the reference belongs to.
    pub session: SessionId,
    /// Unix wall time, in milliseconds, on the session's common clock.
    pub t_unix_ms: i64,
    /// The scope the reference covers.
    pub scope: EventScope,
    /// The typed reference payload.
    pub payload: EventPayload,
}

/// How much of a declared scope a source actually covers (`evaluation.md` §3.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Coverage {
    /// The number of scope cells actually labelled.
    pub labelled: usize,
    /// The size of the declared scope.
    pub total: usize,
    /// A human-readable note, for the ledger.
    pub notes: String,
}

impl Coverage {
    /// The coverage fraction in `[0, 1]`; `1.0` for an empty scope.
    #[must_use]
    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            1.0
        } else {
            self.labelled as f64 / self.total as f64
        }
    }
}

/// The provenance of a reference source, for the ledger (`evaluation.md` §2.3).
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceProvenance {
    /// The adapter name.
    pub source: String,
    /// The reference kind the adapter labels.
    pub kind: GtKind,
    /// The number of reference records.
    pub records: usize,
    /// A content digest over the canonical record stream.
    pub digest: [u8; 32],
    /// Free-form calibration / alignment notes.
    pub notes: String,
}

impl ReferenceProvenance {
    /// The digest as lower-case hexadecimal.
    #[must_use]
    pub fn digest_hex(&self) -> String {
        to_hex(&self.digest)
    }
}

/// The declared evaluation scope (`evaluation.md` §3.1).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EvalScope {
    /// The sessions the claim covers.
    pub sessions: Vec<SessionId>,
    /// The rooms the claim covers.
    pub rooms: Vec<RoomId>,
}

/// An offline reference label source. Never a runtime input.
pub trait GroundTruth {
    /// What this source can label.
    fn kind(&self) -> ReferenceKind;

    /// The reference record for a time window of a session, if one exists.
    ///
    /// An adapter that cannot align returns `None` with a coverage gap, never an
    /// interpolated guess.
    fn label(&self, session: &SessionId, window: TimeWindow) -> Option<ReferenceRecord>;

    /// Which part of the declared scope this source actually covers.
    fn coverage(&self, scope: &EvalScope) -> Coverage;

    /// Provenance — source, calibration and digest — for the ledger.
    fn provenance(&self) -> ReferenceProvenance;
}

/// The v1 default reference adapter: labels committed with a fixture set, or
/// synthesised by the simulator (`evaluation.md` §2.2).
#[derive(Debug, Clone)]
pub struct FixtureGroundTruth {
    kind: ReferenceKind,
    session: SessionId,
    records: Vec<ReferenceRecord>,
    provenance: ReferenceProvenance,
}

impl FixtureGroundTruth {
    /// Build a reference adapter from a typed record stream.
    ///
    /// Records are filtered to `kind`, so a presence adapter ignores count and
    /// vitals rows.
    #[must_use]
    pub fn from_records(
        session: SessionId,
        kind: ReferenceKind,
        records: Vec<GroundTruthRecord>,
    ) -> Self {
        let mut digest_input: Vec<u8> = Vec::new();
        for record in &records {
            if let Ok(bytes) = serde_json::to_vec(record) {
                digest_input.extend_from_slice(&bytes);
            }
        }
        let digest = sha256(&digest_input);

        let mut filtered: Vec<ReferenceRecord> = records
            .into_iter()
            .filter(|record| kind.matches(&record.payload))
            .map(|record| ReferenceRecord {
                session: record.session,
                t_unix_ms: record.t_unix_ms,
                scope: record.scope,
                payload: record.payload,
            })
            .collect();
        filtered.sort_by_key(|record| record.t_unix_ms);

        let provenance = ReferenceProvenance {
            source: "FixtureGroundTruth".to_string(),
            kind: GtKind::Synthetic,
            records: filtered.len(),
            digest,
            notes: format!("{kind:?} reference"),
        };
        Self {
            kind,
            session,
            records: filtered,
            provenance,
        }
    }

    /// A presence reference adapter.
    #[must_use]
    pub fn presence(session: SessionId, records: Vec<GroundTruthRecord>) -> Self {
        Self::from_records(session, ReferenceKind::Presence, records)
    }

    /// A count reference adapter.
    #[must_use]
    pub fn count(session: SessionId, records: Vec<GroundTruthRecord>) -> Self {
        Self::from_records(session, ReferenceKind::Count, records)
    }

    /// Build a presence reference directly from a scene's synthesised ground
    /// truth (`capture-and-firmware.md` §7.3).
    ///
    /// This is the replay-and-simulation-first path (`evaluation.md` §1): no
    /// nodes and no hardware are needed to obtain a reference.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationError::Simulator`] when the scene fails validation.
    pub fn presence_from_scene(
        script: &SceneScript,
        config: &SimConfig,
    ) -> Result<Self, EvaluationError> {
        let source = SimulatorSource::new(script.clone(), config.clone())?;
        let records: Vec<GroundTruthRecord> = source
            .ground_truth()
            .into_iter()
            .map(|(_, record)| record)
            .collect();
        Ok(Self::presence(config.session.clone(), records))
    }

    /// The number of filtered reference records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether no reference records are held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The session this reference belongs to.
    #[must_use]
    pub const fn session(&self) -> &SessionId {
        &self.session
    }
}

impl GroundTruth for FixtureGroundTruth {
    fn kind(&self) -> ReferenceKind {
        self.kind
    }

    fn label(&self, session: &SessionId, window: TimeWindow) -> Option<ReferenceRecord> {
        if session != &self.session {
            return None;
        }
        self.records
            .iter()
            .find(|record| {
                let ns = record.t_unix_ms.max(0) as u64 * 1_000_000;
                window.contains(aquila_types::CoreTime(ns))
            })
            .cloned()
    }

    fn coverage(&self, scope: &EvalScope) -> Coverage {
        let labelled: BTreeSet<&RoomId> = self
            .records
            .iter()
            .filter_map(|record| match &record.scope {
                EventScope::Room(room) => Some(room),
                _ => None,
            })
            .collect();
        let total = if scope.rooms.is_empty() {
            labelled.len()
        } else {
            scope.rooms.len()
        };
        Coverage {
            labelled: labelled.len().min(total),
            total,
            notes: "rooms with at least one reference record".to_string(),
        }
    }

    fn provenance(&self) -> ReferenceProvenance {
        self.provenance.clone()
    }
}
