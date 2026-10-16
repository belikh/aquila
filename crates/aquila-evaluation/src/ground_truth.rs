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
use std::path::{Path, PathBuf};

use aquila_simulator::{SceneScript, SimConfig, SimulatorSource};
use aquila_types::{
    sha256, to_hex, CoreTime, EventPayload, EventScope, GroundTruthRecord, GtKind, RoomId,
    SessionId, TimeWindow,
};
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
                EventScope::Zone(room) => Some(room),
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

/// A vitals reference adapter backed by a JSONL file of [`ReferenceRecord`]s.
///
/// Hardware-gated: until a respiratory belt (breathing) or chest-strap ECG
/// (heart) is connected, this adapter must **never fabricate a value**. The
/// reference records are read from a JSONL file configured at construction —
/// one record per line, matching [`ReferenceRecord`] shape. When the file is
/// absent the adapter holds no records and reports coverage 0; malformed lines
/// are skipped, never interpreted as a measurement.
///
/// This is scaffolding for v1: unverified until the respiratory belt / chest
/// strap reference hardware arrives.
#[derive(Debug, Clone)]
pub struct ReferenceVitalsGroundTruth {
    kind: ReferenceKind,
    path: PathBuf,
    records: Vec<ReferenceRecord>,
    provenance: ReferenceProvenance,
}

impl ReferenceVitalsGroundTruth {
    /// Build a vitals reference adapter from a JSONL file path.
    ///
    /// `kind` must be [`ReferenceKind::Breathing`] or [`ReferenceKind::Heart`];
    /// records whose payload does not match `kind` are dropped. Records are
    /// sorted by `t_unix_ms` so [`label`] can scan in time order.
    ///
    /// If the file does not exist, the adapter holds no records and reports
    /// coverage 0 — it never synthesises a breathing or heart rate.
    #[must_use]
    pub fn new(kind: ReferenceKind, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let notes = format!(
            "{kind:?} reference from JSONL at {}",
            path.display()
        );
        let records = Self::load_records(&path, kind);
        let digest = digest_of_records(&records);
        let provenance = ReferenceProvenance {
            source: "ReferenceVitalsGroundTruth".to_string(),
            kind: GtKind::ReferenceSensor,
            records: records.len(),
            digest,
            notes,
        };
        Self {
            kind,
            path,
            records,
            provenance,
        }
    }

    /// Read and parse the JSONL file, returning the matching records.
    ///
    /// An absent file yields zero records. A line that does not parse as a
    /// [`ReferenceRecord`], or whose payload does not match `kind`, is skipped
    /// — never treated as a measurement.
    fn load_records(path: &Path, kind: ReferenceKind) -> Vec<ReferenceRecord> {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => return Vec::new(),
        };
        let mut records: Vec<ReferenceRecord> = content
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    return None;
                }
                serde_json::from_str::<ReferenceRecord>(line).ok()
            })
            .filter(|record| kind.matches(&record.payload))
            .collect();
        records.sort_by_key(|record| record.t_unix_ms);
        records
    }

    /// The number of filtered reference records held by this adapter.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether no reference records are held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The JSONL file path this adapter reads from.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl GroundTruth for ReferenceVitalsGroundTruth {
    fn kind(&self) -> ReferenceKind {
        self.kind
    }

    fn label(&self, session: &SessionId, window: TimeWindow) -> Option<ReferenceRecord> {
        self.records
            .iter()
            .find(|record| {
                &record.session == session
                    && window.contains(CoreTime(record.t_unix_ms.max(0) as u64 * 1_000_000))
            })
            .cloned()
    }

    fn coverage(&self, scope: &EvalScope) -> Coverage {
        let labelled: BTreeSet<&RoomId> = self
            .records
            .iter()
            .filter_map(|record| match &record.scope {
                EventScope::Zone(room) => Some(room),
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
            notes: "rooms with at least one vitals reference record".to_string(),
        }
    }

    fn provenance(&self) -> ReferenceProvenance {
        self.provenance.clone()
    }
}

/// The content digest over a canonical record stream, reusing the workspace
/// SHA-256 primitive so provenance is reproducible.
fn digest_of_records(records: &[ReferenceRecord]) -> [u8; 32] {
    let mut input: Vec<u8> = Vec::new();
    for record in records {
        if let Ok(bytes) = serde_json::to_vec(record) {
            input.extend_from_slice(&bytes);
        }
    }
    sha256(&input)
}

/// A skeleton MM-Fi pose reference adapter.
///
/// Labels torso pose from MM-Fi-style labels captured by the research camera
/// rig. Hardware-gated: neither the MM-Fi corpus nor the research camera rig
/// exists in v1, so this adapter reports **no labels and coverage 0** until
/// both arrive. It never fabricates a pose estimate.
///
/// This is scaffolding for v1: unverified until the MM-Fi corpus / research
/// camera rig reference arrives.
#[derive(Debug, Clone)]
pub struct MmFiLabelGroundTruth {
    kind: ReferenceKind,
    provenance: ReferenceProvenance,
}

impl MmFiLabelGroundTruth {
    /// Build the skeleton pose adapter.
    ///
    /// No labels are available yet: the adapter reports coverage 0 and
    /// [`GroundTruth::label`] always returns `None`.
    #[must_use]
    pub fn new() -> Self {
        let provenance = ReferenceProvenance {
            source: "MmFiLabelGroundTruth".to_string(),
            kind: GtKind::CameraResearch,
            records: 0,
            digest: sha256(&[]),
            notes: "skeleton: awaiting MM-Fi corpus and research camera rig; no labels yet"
                .to_string(),
        };
        Self {
            kind: ReferenceKind::Pose,
            provenance,
        }
    }
}

impl GroundTruth for MmFiLabelGroundTruth {
    fn kind(&self) -> ReferenceKind {
        self.kind
    }

    fn label(&self, _session: &SessionId, _window: TimeWindow) -> Option<ReferenceRecord> {
        None
    }

    fn coverage(&self, _scope: &EvalScope) -> Coverage {
        Coverage {
            labelled: 0,
            total: 0,
            notes: "no MM-Fi labels available yet".to_string(),
        }
    }

    fn provenance(&self) -> ReferenceProvenance {
        self.provenance.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use aquila_types::Slug;

    fn session(value: &str) -> SessionId {
        Slug(value.to_string())
    }

    fn room(value: &str) -> RoomId {
        Slug(value.to_string())
    }

    fn breathing_record(t_unix_ms: i64, session: &SessionId) -> ReferenceRecord {
        ReferenceRecord {
            session: session.clone(),
            t_unix_ms,
            scope: EventScope::Zone(room("lounge")),
            payload: EventPayload::Vitals {
                metric: "breathing_rate".to_string(),
                value: 16.0,
                unit: "brpm".to_string(),
            },
        }
    }

    #[test]
    fn absent_file_reports_no_labels() {
        let session = session("eval-session");
        let missing = std::env::temp_dir().join("no-such-vitals-9f2d3a.jsonl");
        let truth = ReferenceVitalsGroundTruth::new(ReferenceKind::Breathing, missing);
        assert_eq!(truth.kind(), ReferenceKind::Breathing);
        assert!(truth.is_empty());
        assert_eq!(truth.provenance().records, 0);

        let window =
            TimeWindow::new(CoreTime(0), CoreTime(100_000_000_000)).expect("window");
        assert!(truth.label(&session, window).is_none());

        let scope = EvalScope {
            sessions: vec![session.clone()],
            rooms: vec![room("lounge")],
        };
        let coverage = truth.coverage(&scope);
        assert_eq!(coverage.labelled, 0);
        assert_eq!(coverage.total, 1);
        assert_eq!(coverage.fraction(), 0.0);
    }

    #[test]
    fn present_file_returns_records() {
        let session = session("eval-session");
        let path = std::env::temp_dir().join("vitals-present.jsonl");
        let record = breathing_record(10_000, &session);
        let line = serde_json::to_string(&record).expect("serialize record");
        std::fs::write(&path, format!("{line}\n")).expect("write jsonl");
        let truth = ReferenceVitalsGroundTruth::new(ReferenceKind::Breathing, path.clone());
        let _ = std::fs::remove_file(&path);

        assert_eq!(truth.provenance().records, 1);
        assert!(!truth.is_empty());

        let window = TimeWindow::new(
            CoreTime(10_000 * 1_000_000),
            CoreTime(10_001 * 1_000_000),
        )
        .expect("window");
        let got = truth.label(&session, window).expect("label present");
        assert_eq!(got.t_unix_ms, 10_000);
        assert!(matches!(
            got.payload,
            EventPayload::Vitals { metric, .. } if metric == "breathing_rate"
        ));

        let scope = EvalScope {
            sessions: vec![session.clone()],
            rooms: vec![room("lounge")],
        };
        let coverage = truth.coverage(&scope);
        assert_eq!(coverage.labelled, 1);
        assert_eq!(coverage.total, 1);
        assert_eq!(coverage.fraction(), 1.0);
    }

    #[test]
    fn mmfi_skeleton_reports_no_labels() {
        let truth = MmFiLabelGroundTruth::new();
        assert_eq!(truth.kind(), ReferenceKind::Pose);
        assert_eq!(truth.provenance().records, 0);

        let window =
            TimeWindow::new(CoreTime(0), CoreTime(1_000_000_000)).expect("window");
        assert!(truth.label(&session("x"), window).is_none());

        let coverage = truth.coverage(&EvalScope::default());
        assert_eq!(coverage.labelled, 0);
        assert_eq!(coverage.total, 0);
    }
}
