//! Pure view-models and form validation (`docs/spec/dashboard.md` §6, §9).
//!
//! These functions turn the core's JSON resources into the shapes the screens
//! render, and they validate operator input before it is sent. They perform no
//! DSP, inference or persistence (`dashboard.md` §1) and are the host-testable
//! heart of the UI: the Leptos components are thin over them, so the behaviour
//! acceptance criteria of ticket #10 can be checked without a browser.
//!
//! # Prose
//!
//! Australian English throughout (normalise, serialisation, behaviour).

use crate::contract::{Health, NewRoom, Node, NodePair, Room, Schema, Snapshot};

/// A floor with its rooms, ordered for display.
#[derive(Debug, Clone, PartialEq)]
pub struct FloorView {
    /// `flr_...`.
    pub id: String,
    /// Display name.
    pub name: String,
    /// The floor level (`-1` basement, `0` ground).
    pub level: i8,
    /// The rooms on the floor, ordered by name.
    pub rooms: Vec<RoomView>,
}

/// A room as shown in the structure tree.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomView {
    /// `rom_...`.
    pub id: String,
    /// Display name.
    pub name: String,
    /// The room kind/label.
    pub kind: String,
}

/// The whole structure screen view-model.
#[derive(Debug, Clone, PartialEq)]
pub struct StructureView {
    /// The home name.
    pub home_name: String,
    /// The floors, ordered by level then name.
    pub floors: Vec<FloorView>,
    /// The total number of rooms.
    pub room_count: usize,
}

/// Build the structure view-model from `GET /schema`.
///
/// Floors are ordered by `level` (then name), and rooms within a floor by name,
/// so the tree is stable across polls. Rooms whose `floor_id` matches no floor
/// are dropped rather than invented into a phantom floor.
#[must_use]
pub fn structure_view(schema: &Schema) -> StructureView {
    let mut floors: Vec<FloorView> = schema
        .floors
        .iter()
        .map(|floor| FloorView {
            id: floor.id.clone(),
            name: if floor.name.is_empty() {
                floor.id.clone()
            } else {
                floor.name.clone()
            },
            level: floor.level,
            rooms: Vec::new(),
        })
        .collect();
    floors.sort_by(|a, b| a.level.cmp(&b.level).then_with(|| a.name.cmp(&b.name)));

    for room in &schema.rooms {
        if let Some(floor) = floors.iter_mut().find(|floor| floor.id == room.floor_id) {
            floor.rooms.push(room_view(room));
        }
    }
    for floor in &mut floors {
        floor.rooms.sort_by(|a, b| a.name.cmp(&b.name));
    }

    let room_count = floors.iter().map(|floor| floor.rooms.len()).sum();
    StructureView {
        home_name: if schema.home.name.is_empty() {
            "Home".to_string()
        } else {
            schema.home.name.clone()
        },
        floors,
        room_count,
    }
}

fn room_view(room: &Room) -> RoomView {
    RoomView {
        id: room.id.clone(),
        name: if room.name.is_empty() {
            room.id.clone()
        } else {
            room.name.clone()
        },
        kind: if room.kind.is_empty() {
            "other".to_string()
        } else {
            room.kind.clone()
        },
    }
}

/// A field-level validation error, tied to a control by `aria-describedby`
/// (`dashboard.md` §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    /// The control this error describes (`name`, `floor`, `label`).
    pub field: &'static str,
    /// The inline message.
    pub message: String,
}

/// Validate the create-room form (`dashboard.md` §6, Structure).
///
/// The name must be non-empty after trimming and at most 64 characters; a
/// floor must be selected. The label defaults to `other` when blank
/// (`core-api.md` §7).
///
/// # Errors
///
/// Returns every failing [`FieldError`] so the form can show them all at once.
pub fn validate_new_room(
    floor_id: &str,
    name: &str,
    label: &str,
) -> Result<NewRoom, Vec<FieldError>> {
    let mut errors = Vec::new();
    if floor_id.trim().is_empty() {
        errors.push(FieldError {
            field: "floor",
            message: "Choose a floor for the room.".to_string(),
        });
    }
    let trimmed = name.trim();
    if trimmed.is_empty() {
        errors.push(FieldError {
            field: "name",
            message: "Enter a room name.".to_string(),
        });
    } else if trimmed.chars().count() > 64 {
        errors.push(FieldError {
            field: "name",
            message: "Keep the room name to 64 characters or fewer.".to_string(),
        });
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let label = label.trim();
    Ok(NewRoom {
        name: trimmed.to_string(),
        label: if label.is_empty() {
            "other".to_string()
        } else {
            label.to_string()
        },
    })
}

/// One room's live readings, as shown on the overview and room screens.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneView {
    /// The room id (the snapshot's zone key).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Presence, or `None` when unknown.
    pub presence: Option<bool>,
    /// Person count, or `None`.
    pub count: Option<u16>,
    /// Breathing rate in brpm, or `None`.
    pub breathing: Option<f32>,
    /// Heart rate in bpm, or `None`.
    pub heart: Option<f32>,
}

impl ZoneView {
    /// A presence label that never relies on colour alone (`dashboard.md` §7).
    #[must_use]
    pub fn presence_label(&self) -> &'static str {
        match self.presence {
            Some(true) => "Present",
            Some(false) => "Absent",
            None => "Unknown",
        }
    }
}

/// The overview / live view-model.
#[derive(Debug, Clone, PartialEq)]
pub struct OverviewView {
    /// Whether any room currently reports presence.
    pub occupied: bool,
    /// The summed person count across rooms.
    pub people: u16,
    /// Per-room readings, sorted by name.
    pub rooms: Vec<ZoneView>,
    /// Whether any reading is known at all.
    pub has_data: bool,
}

/// Build the overview view-model from `GET /state`.
#[must_use]
pub fn overview_view(snapshot: &Snapshot) -> OverviewView {
    let mut rooms: Vec<ZoneView> = snapshot
        .zones
        .iter()
        .map(|(id, zone)| ZoneView {
            id: id.clone(),
            name: if zone.name.is_empty() {
                id.clone()
            } else {
                zone.name.clone()
            },
            presence: zone.presence,
            count: zone.person_count,
            breathing: zone.breathing_rate,
            heart: zone.heart_rate,
        })
        .collect();
    rooms.sort_by(|a, b| a.name.cmp(&b.name));

    let occupied = rooms.iter().any(|room| room.presence == Some(true));
    let people = rooms
        .iter()
        .filter_map(|room| room.count)
        .fold(0u16, u16::saturating_add);
    let has_data = !rooms.is_empty();
    OverviewView {
        occupied,
        people,
        rooms,
        has_data,
    }
}

/// The throttled, summarised live-region text (`dashboard.md` §7).
///
/// One polite sentence, derived from the snapshot, that a screen reader can
/// announce without being flooded by per-poll telemetry.
#[must_use]
pub fn live_summary(overview: &OverviewView) -> String {
    if !overview.has_data {
        return "No rooms reporting yet.".to_string();
    }
    let present: Vec<&str> = overview
        .rooms
        .iter()
        .filter(|room| room.presence == Some(true))
        .map(|room| room.name.as_str())
        .collect();
    if present.is_empty() {
        "No presence detected.".to_string()
    } else {
        format!(
            "{} {}: {}.",
            overview.people,
            if overview.people == 1 {
                "person"
            } else {
                "people"
            },
            present.join(", ")
        )
    }
}

/// The system-health view-model (`dashboard.md` §6, Health).
#[derive(Debug, Clone, PartialEq)]
pub struct HealthView {
    /// The worst subsystem status.
    pub status: String,
    /// A one-line summary.
    pub summary: String,
}

/// Build the health view-model from `GET /health`.
#[must_use]
pub fn health_view(health: &Health) -> HealthView {
    let degraded = health
        .subsystems
        .iter()
        .filter(|subsystem| subsystem.status != "ok")
        .count();
    let summary = if health.subsystems.is_empty() {
        "No subsystem reports available.".to_string()
    } else if degraded == 0 {
        format!("All {} subsystems healthy.", health.subsystems.len())
    } else {
        format!(
            "{degraded} of {} subsystems need attention.",
            health.subsystems.len()
        )
    };
    HealthView {
        status: if health.status.is_empty() {
            "unknown".to_string()
        } else {
            health.status.clone()
        },
        summary,
    }
}

/// Count nodes by state for the nodes screen.
#[must_use]
pub fn node_state_counts(nodes: &[Node]) -> Vec<(String, usize)> {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for node in nodes {
        let state = if node.state.is_empty() {
            "unknown"
        } else {
            node.state.as_str()
        };
        *counts.entry(state.to_string()).or_default() += 1;
    }
    counts.into_iter().collect()
}

/// The state class used by the status pill and badges (`dashboard.md` §7).
///
/// Never the sole carrier of meaning: every caller pairs it with an icon and
/// text.
#[must_use]
pub fn state_class(state: &str) -> &'static str {
    match state {
        "ok" | "online" | "capturing" => "ok",
        "degraded" | "warn" => "degraded",
        "down" | "offline" | "error" => "down",
        _ => "unknown",
    }
}

/// Render a node pair as `a ↔ b`, with a stable arrow.
#[must_use]
pub fn pair_label(pair: &NodePair) -> String {
    format!("{} \u{2194} {}", pair.node_a, pair.node_b)
}

/// Format an optional metric value with its unit, using an em dash for an
/// unknown reading (never a fabricated zero, `dashboard.md` §6).
#[must_use]
pub fn format_metric(value: Option<f32>, unit: &str) -> String {
    match value {
        Some(value) => {
            if unit.is_empty() {
                format!("{value:.1}")
            } else {
                format!("{value:.1} {unit}")
            }
        }
        None => "\u{2014}".to_string(),
    }
}

/// Format an optional person count, using an em dash when unknown.
#[must_use]
pub fn format_count(value: Option<u16>) -> String {
    match value {
        Some(count) => count.to_string(),
        None => "\u{2014}".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{Floor, Room, SchemaHome};
    use std::collections::BTreeMap;

    fn fixture_schema() -> Schema {
        Schema {
            instance_id: "inst".to_string(),
            contract: "1.0".to_string(),
            home: SchemaHome {
                id: "hom_1".to_string(),
                name: "Test Home".to_string(),
                time_zone: "Australia/Sydney".to_string(),
            },
            floors: vec![
                Floor {
                    id: "flr_up".to_string(),
                    name: "Upstairs".to_string(),
                    level: 1,
                    elevation_m: 3.0,
                },
                Floor {
                    id: "flr_ground".to_string(),
                    name: "Ground".to_string(),
                    level: 0,
                    elevation_m: 0.0,
                },
            ],
            rooms: vec![
                Room {
                    id: "rom_study".to_string(),
                    floor_id: "flr_up".to_string(),
                    name: "Study".to_string(),
                    kind: "study".to_string(),
                },
                Room {
                    id: "rom_kitchen".to_string(),
                    floor_id: "flr_ground".to_string(),
                    name: "Kitchen".to_string(),
                    kind: "kitchen".to_string(),
                },
                Room {
                    id: "rom_living".to_string(),
                    floor_id: "flr_ground".to_string(),
                    name: "Living".to_string(),
                    kind: "living".to_string(),
                },
            ],
            metrics: Vec::new(),
        }
    }

    #[test]
    fn the_structure_view_renders_floors_and_rooms_in_order() {
        let view = structure_view(&fixture_schema());
        assert_eq!(view.home_name, "Test Home");
        assert_eq!(view.room_count, 3);
        // Ground (level 0) sorts before Upstairs (level 1).
        assert_eq!(view.floors[0].name, "Ground");
        assert_eq!(view.floors[1].name, "Upstairs");
        // Rooms within a floor sort by name: Kitchen before Living.
        let ground: Vec<&str> = view.floors[0]
            .rooms
            .iter()
            .map(|room| room.name.as_str())
            .collect();
        assert_eq!(ground, ["Kitchen", "Living"]);
        assert_eq!(view.floors[1].rooms[0].name, "Study");
    }

    #[test]
    fn creating_a_room_requires_a_name_and_a_floor() {
        let errors = validate_new_room("", "  ", "").expect_err("must fail");
        let fields: Vec<&str> = errors.iter().map(|error| error.field).collect();
        assert!(fields.contains(&"floor"));
        assert!(fields.contains(&"name"));

        let room = validate_new_room("flr_ground", " Kitchen ", "").expect("valid");
        assert_eq!(room.name, "Kitchen");
        assert_eq!(room.label, "other");
    }

    #[test]
    fn an_overlong_room_name_is_rejected() {
        let long = "x".repeat(65);
        assert!(validate_new_room("flr_ground", &long, "").is_err());
    }

    #[test]
    fn the_live_summary_is_one_polite_sentence() {
        let mut zones = BTreeMap::new();
        zones.insert(
            "rom_kitchen".to_string(),
            crate::contract::Zone {
                kind: "room".to_string(),
                name: "Kitchen".to_string(),
                presence: Some(true),
                person_count: Some(2),
                ..Default::default()
            },
        );
        let snapshot = Snapshot {
            instance_id: "inst".to_string(),
            home: "hom_1".to_string(),
            zones,
        };
        let view = overview_view(&snapshot);
        assert!(view.occupied);
        assert_eq!(view.people, 2);
        assert_eq!(live_summary(&view), "2 people: Kitchen.");
        assert_eq!(view.rooms[0].presence_label(), "Present");
    }

    #[test]
    fn an_empty_snapshot_is_an_honest_empty_state() {
        let view = overview_view(&Snapshot::default());
        assert!(!view.has_data);
        assert_eq!(live_summary(&view), "No rooms reporting yet.");
    }
}
