//! `Detection` and `Event`: two distinct concepts, deliberately not merged.
//!
//! A [`Detection`] is a *stateless* inference for a zone or the home over a
//! span. It is emitted every span and may say "no change". An [`Event`] is an
//! *edge-triggered, deduplicated* state change destined for the `Sink`, emitted
//! only when state actually changes, after hysteresis. A `Detection` is the
//! evidence; an `Event` is the notification.

use serde::{Deserialize, Serialize};

use crate::error::{require_finite_f32, require_non_empty, require_unit_interval, ValidationError};
use crate::ids::{HomeId, NodeId, ZoneId};
use crate::primitives::Witness;
use crate::time::TimeSpan;

/// The v1 cap on a people count.
pub const MAX_COUNT_V1: u8 = 8;

/// The kind of a detection. `Unknown` is a first-class fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionKind {
    /// A per-zone binary claim that at least one person is present.
    Presence,
    /// People attributed to a zone or the home.
    Count,
    /// The whole-home fused state of which zones are occupied, and by how many.
    Occupancy,
    /// An unrecognised or unset detection kind.
    #[serde(other)]
    Unknown = 255,
}

/// The scope of a detection: the home, or a zone (a virtual detection region).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionScope {
    /// The whole home.
    Home(HomeId),
    /// A zone, for example "bed" inside the bedroom or "shower" inside the
    /// bathroom. A zone may span rooms or be coextensive with one.
    Zone(ZoneId),
}

impl DetectionScope {
    /// Validate that the carried identifier is non-empty.
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            DetectionScope::Home(id) => id.validate(),
            DetectionScope::Zone(id) => id.validate(),
        }
    }
}

/// The value of a detection, typed to match its [`DetectionKind`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionValue {
    /// Presence probability in `[0, 1]`.
    Probability(f32),
    /// A people count.
    Count(u8),
    /// Per-zone people counts, for an occupancy detection.
    Occupancy(Vec<(ZoneId, u8)>),
}

/// A stateless inference for a zone or the home over a span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Detection {
    /// The scope: home or room.
    pub scope: DetectionScope,
    /// The span the detection covers.
    pub span: TimeSpan,
    /// The kind of detection.
    pub kind: DetectionKind,
    /// Confidence in `[0, 1]`.
    pub confidence: f32,
    /// The typed value.
    pub value: DetectionValue,
    /// The contributing nodes; non-empty, every element in the topology.
    pub contributors: Vec<NodeId>,
    /// The evidence grade.
    pub witness: Witness,
}

impl Detection {
    /// Validate contributors, confidence, and the value/kind/scope coupling.
    ///
    /// * `Presence` requires a `Probability` value and a `Room` scope.
    /// * `Count` requires a `Count` value.
    /// * `Occupancy` requires an `Occupancy` value and a `Home` scope.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.scope.validate()?;
        self.span.validate()?;
        require_unit_interval("Detection.confidence", self.confidence)?;
        if self.contributors.is_empty() {
            return Err(ValidationError::Invariant {
                detail: "Detection.contributors must be non-empty".to_string(),
            });
        }
        for contributor in &self.contributors {
            contributor.validate_provisioned()?;
        }

        match (&self.kind, &self.value, &self.scope) {
            (DetectionKind::Presence, DetectionValue::Probability(p), DetectionScope::Zone(_)) => {
                require_unit_interval("DetectionValue::Probability", *p)
            }
            (DetectionKind::Count, DetectionValue::Count(n), _) => {
                if *n <= MAX_COUNT_V1 {
                    Ok(())
                } else {
                    Err(ValidationError::Range {
                        field: "DetectionValue::Count",
                        detail: format!("{n} exceeds the v1 cap of {MAX_COUNT_V1}"),
                    })
                }
            }
            (
                DetectionKind::Occupancy,
                DetectionValue::Occupancy(by_zone),
                DetectionScope::Home(_),
            ) => {
                for (room, count) in by_zone {
                    room.validate()?;
                    if *count > MAX_COUNT_V1 {
                        return Err(ValidationError::Range {
                            field: "DetectionValue::Occupancy",
                            detail: format!("count {count} exceeds the v1 cap of {MAX_COUNT_V1}"),
                        });
                    }
                }
                Ok(())
            }
            _ => Err(ValidationError::Invariant {
                detail: "Detection.value does not match Detection.kind and scope".to_string(),
            }),
        }
    }
}

/// The kind of an edge-triggered event. `Unknown` is a first-class fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// A zone's presence state changed.
    PresenceChanged,
    /// A count changed.
    CountChanged,
    /// The home occupancy changed.
    OccupancyChanged,
    /// A person entered a zone.
    PersonEntered,
    /// A person left a zone.
    PersonLeft,
    /// A vitals alert fired.
    VitalAlert,
    /// Calibration changed or was invalidated.
    CalibrationChanged,
    /// A node came online.
    NodeOnline,
    /// A node went offline.
    NodeOffline,
    /// The model changed.
    ModelChanged,
    /// An unrecognised or unset event kind.
    #[serde(other)]
    Unknown = 255,
}

/// The scope of an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventScope {
    /// The whole home.
    Home,
    /// A zone (a virtual detection region).
    Zone(ZoneId),
}

impl EventScope {
    /// Validate that any carried identifier is non-empty.
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            EventScope::Home => Ok(()),
            EventScope::Zone(id) => id.validate(),
        }
    }
}

/// The typed payload of an event, one variant per event kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventPayload {
    /// A presence change, with the new state and its probability.
    Presence {
        /// Whether the zone is now occupied.
        occupied: bool,
        /// The presence probability in `[0, 1]`.
        probability: f32,
    },
    /// A count change.
    Count {
        /// The new people count.
        people: u8,
    },
    /// An occupancy change, per zone.
    Occupancy {
        /// The per-zone people counts.
        by_zone: Vec<(ZoneId, u8)>,
    },
    /// A vitals alert.
    Vitals {
        /// The metric name, for example `"breathing_rate"`.
        metric: String,
        /// The value.
        value: f32,
        /// The unit label, for example `"brpm"`.
        unit: String,
    },
    /// A node lifecycle change.
    Node {
        /// The node.
        node: NodeId,
        /// A human-readable reason.
        reason: String,
    },
    /// A generic change summary.
    Change {
        /// A human-readable summary.
        summary: String,
    },
    /// No payload.
    None,
}

impl EventPayload {
    /// Validate the payload's numeric ranges and non-empty text fields.
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            EventPayload::Presence { probability, .. } => {
                require_unit_interval("EventPayload::Presence.probability", *probability)
            }
            EventPayload::Count { people } => {
                if *people <= MAX_COUNT_V1 {
                    Ok(())
                } else {
                    Err(ValidationError::Range {
                        field: "EventPayload::Count.people",
                        detail: format!("{people} exceeds the v1 cap of {MAX_COUNT_V1}"),
                    })
                }
            }
            EventPayload::Occupancy { by_zone } => {
                for (zone, count) in by_zone {
                    zone.validate()?;
                    if *count > MAX_COUNT_V1 {
                        return Err(ValidationError::Range {
                            field: "EventPayload::Occupancy",
                            detail: format!("count {count} exceeds the v1 cap of {MAX_COUNT_V1}"),
                        });
                    }
                }
                Ok(())
            }
            EventPayload::Vitals {
                metric,
                value,
                unit,
            } => {
                require_non_empty("EventPayload::Vitals.metric", metric)?;
                require_non_empty("EventPayload::Vitals.unit", unit)?;
                require_finite_f32("EventPayload::Vitals.value", *value)
            }
            EventPayload::Node { reason, .. } => {
                require_non_empty("EventPayload::Node.reason", reason)
            }
            EventPayload::Change { summary } => {
                require_non_empty("EventPayload::Change.summary", summary)
            }
            EventPayload::None => Ok(()),
        }
    }
}

/// An edge-triggered, deduplicated state change destined for the `Sink`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Deterministic identity: `hash(home, kind, scope, t_slot)`.
    pub id: String,
    /// The home.
    pub home: HomeId,
    /// Unix wall time, in milliseconds.
    pub t_unix_ms: i64,
    /// Deterministic deduplication key: `"{home}:{kind}:{scope}"`.
    pub dedup_key: String,
    /// The scope.
    pub scope: EventScope,
    /// The kind.
    pub kind: EventKind,
    /// The typed payload.
    pub payload: EventPayload,
    /// The evidence grade.
    pub witness: Witness,
}

impl Event {
    /// Validate identity, dedup key, scope and payload.
    ///
    /// The identity and dedup key are deterministic functions of their inputs;
    /// this method only checks they are present and structurally well formed,
    /// never recomputes them.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_empty("Event.id", &self.id)?;
        require_non_empty("Event.dedup_key", &self.dedup_key)?;
        self.home.validate()?;
        self.scope.validate()?;
        self.payload.validate()
    }
}
