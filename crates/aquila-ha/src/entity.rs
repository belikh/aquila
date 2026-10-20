//! The in-memory HA entity projection.
//!
//! [`EntityModel`] holds the current HA-shaped state; [`EntityModel::apply`]
//! folds one core [`Event`](aquila_types::Event) into it, and
//! [`EntityModel::entities`] / [`EntityModel::snapshot`] render it for the
//! transport (`aquila-api`) to serve. The model is a plain map, so it is
//! testable without a Home Assistant host.
//!
//! Detection results are attributed to **zones** (virtual detection regions
//! such as "bed" inside the bedroom or "shower" versus "toilet" inside the
//! bathroom), never to rooms. Rooms describe where nodes are physically
//! installed; zones describe where a person is detected.

use std::collections::BTreeMap;

use aquila_geometry::Topology;
use aquila_types::{Event, EventPayload, EventScope, HomeId, ZoneId};
use serde_json::{json, Value};

/// The Home Assistant platform an entity belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    /// A `binary_sensor` entity (presence, connectivity).
    BinarySensor,
    /// A `sensor` entity (counts, rates).
    Sensor,
}

/// One projected Home Assistant entity.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityState {
    /// The stable HA `unique_id`: `aquila:{instance}:{zone}:{metric}`.
    pub unique_id: String,
    /// The HA object id, for example `binary_sensor.aquila_bed_presence`.
    pub entity_id: String,
    /// The HA platform.
    pub kind: EntityKind,
    /// The rendered state string (`on`/`off`, a number, or `unknown`).
    pub state: String,
    /// The unit of measurement, when the entity is a measurement.
    pub unit: Option<String>,
    /// The HA `device_class`, when one applies.
    pub device_class: Option<String>,
    /// The HA `entity_category`, when the entity is diagnostic.
    pub entity_category: Option<String>,
}

/// The projected state of one zone.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ZoneState {
    /// Human-readable zone name, for example `"bed"` or `"shower"`.
    pub name: String,
    /// Whether at least one person is present, when known.
    pub present: Option<bool>,
    /// The presence probability in `[0, 1]`, when known.
    pub presence_probability: Option<f32>,
    /// The people count, when known.
    pub count: Option<u8>,
    /// Breathing rate in brpm, when a usable estimate exists.
    pub breathing_brpm: Option<f32>,
    /// Heart rate in bpm, when a usable estimate exists.
    pub heart_bpm: Option<f32>,
}

/// The Home Assistant-shaped projection of the sensing core's output.
#[derive(Debug, Clone)]
pub struct EntityModel {
    instance_id: String,
    home: HomeId,
    zones: BTreeMap<ZoneId, ZoneState>,
    home_occupancy: Option<bool>,
    home_count: Option<u8>,
}

impl EntityModel {
    /// Construct an empty model for one home instance.
    #[must_use]
    pub fn new(home: HomeId, instance_id: impl Into<String>) -> Self {
        Self {
            instance_id: instance_id.into(),
            home,
            zones: BTreeMap::new(),
            home_occupancy: None,
            home_count: None,
        }
    }

    /// Construct a model seeded with the zones in `topology`. Zones are the
    /// only spatial scope; rooms are backend installation metadata only.
    #[must_use]
    pub fn from_topology(topology: &Topology, instance_id: impl Into<String>) -> Self {
        let home = topology.home().id.clone();
        let mut model = Self::new(home, instance_id);
        for zone in &topology.model().zones {
            model
                .zones
                .entry(zone.id.clone())
                .or_insert_with(|| ZoneState { name: zone.name.clone(), ..Default::default() });
        }
        model
    }

    /// The home this model describes.
    #[must_use]
    pub const fn home(&self) -> &HomeId {
        &self.home
    }

    /// The known zones, in canonical zone-id order.
    #[must_use]
    pub const fn zones(&self) -> &BTreeMap<ZoneId, ZoneState> {
        &self.zones
    }

    /// One zone's projected state, if the zone is known.
    #[must_use]
    pub fn zone(&self, zone: &ZoneId) -> Option<&ZoneState> {
        self.zones.get(zone)
    }

    /// The home occupancy state, when known.
    #[must_use]
    pub const fn home_occupancy(&self) -> Option<bool> {
        self.home_occupancy
    }

    /// The home people count, when known.
    #[must_use]
    pub const fn home_count(&self) -> Option<u8> {
        self.home_count
    }

    /// Fold one core event into the projection.
    ///
    /// Unknown event kinds, unscoped payloads and floor-scoped events are
    /// ignored rather than guessed (`home-assistant.md` §6.1: a `null` reading
    /// stays unknown).
    pub fn apply(&mut self, event: &Event) {
        match (&event.scope, &event.payload) {
            (
                EventScope::Zone(zone),
                EventPayload::Presence {
                    occupied,
                    probability,
                },
            ) => {
                let zone_state = self.zones.entry(zone.clone()).or_default();
                zone_state.present = Some(*occupied);
                zone_state.presence_probability = Some(*probability);
            }
            (EventScope::Zone(zone), EventPayload::Count { people }) => {
                self.zones
                    .entry(zone.clone())
                    .or_default()
                    .count = Some(*people);
            }
            (EventScope::Zone(zone), EventPayload::Vitals { metric, value, .. }) => {
                let zone_state = self.zones.entry(zone.clone()).or_default();
                match metric.as_str() {
                    "breathing_rate" => zone_state.breathing_brpm = Some(*value),
                    "heart_rate" => zone_state.heart_bpm = Some(*value),
                    _ => {}
                }
            }
            (EventScope::Home, EventPayload::Occupancy { by_zone }) => {
                let mut total: u16 = 0;
                for (zone, count) in by_zone {
                    self.zones
                        .entry(zone.clone())
                        .or_default()
                        .count = Some(*count);
                    total = total.saturating_add(u16::from(*count));
                }
                self.home_count = Some(total.min(u16::from(u8::MAX)) as u8);
                self.home_occupancy = Some(total > 0);
            }
            (EventScope::Home, EventPayload::Presence { occupied, .. }) => {
                self.home_occupancy = Some(*occupied);
            }
            (EventScope::Home, EventPayload::Count { people }) => {
                self.home_count = Some(*people);
            }
            _ => {}
        }
    }

    /// Render every known entity, home entities first, then zones in canonical
    /// order.
    #[must_use]
    pub fn entities(&self) -> Vec<EntityState> {
        let mut entities = Vec::new();
        let instance = &self.instance_id;

        if let Some(occupied) = self.home_occupancy {
            entities.push(EntityState {
                unique_id: format!("aquila:{instance}:home:occupancy"),
                entity_id: "binary_sensor.aquila_occupancy".to_string(),
                kind: EntityKind::BinarySensor,
                state: on_off(occupied).to_string(),
                unit: None,
                device_class: Some("occupancy".to_string()),
                entity_category: None,
            });
        }
        if let Some(count) = self.home_count {
            entities.push(EntityState {
                unique_id: format!("aquila:{instance}:home:person_count"),
                entity_id: "sensor.aquila_person_count".to_string(),
                kind: EntityKind::Sensor,
                state: count.to_string(),
                unit: Some("people".to_string()),
                device_class: None,
                entity_category: None,
            });
        }

        for (zone, zone_state) in &self.zones {
            let zone_id = ha_slug(zone);
            if let Some(present) = zone_state.present {
                entities.push(EntityState {
                    unique_id: format!("aquila:{instance}:{zone_id}:presence"),
                    entity_id: format!("binary_sensor.aquila_{zone_id}_presence"),
                    kind: EntityKind::BinarySensor,
                    state: on_off(present).to_string(),
                    unit: None,
                    device_class: Some("presence".to_string()),
                    entity_category: None,
                });
            }
            if let Some(breathing) = zone_state.breathing_brpm {
                entities.push(zone_measurement(
                    instance,
                    &zone_id,
                    "breathing_rate",
                    breathing,
                    "brpm",
                ));
            }
            if let Some(heart) = zone_state.heart_bpm {
                entities.push(zone_measurement(
                    instance,
                    &zone_id,
                    "heart_rate",
                    heart,
                    "bpm",
                ));
            }
        }
        entities
    }

    /// The coordinator snapshot the HA component consumes
    /// (`home-assistant.md` §6), as JSON.
    #[must_use]
    pub fn snapshot(&self) -> Value {
        let mut zones = serde_json::Map::new();
        for (zone, zone_state) in &self.zones {
            zones.insert(
                zone.as_str().to_string(),
                json!({
                    "kind": "zone",
                    "name": zone_state.name,
                    "presence": zone_state.present,
                    "presence_confidence": zone_state.presence_probability,
                    "person_count": zone_state.count,
                    "breathing_rate": zone_state.breathing_brpm,
                    "heart_rate": zone_state.heart_bpm,
                }),
            );
        }
        json!({
            "instance_id": self.instance_id,
            "home": self.home.as_str(),
            "zones": Value::Object(zones),
            "health": {
                "core_connected": true,
                "nodes_online": Value::Null,
                "nodes_total": Value::Null,
                "calibration_state": "unknown",
            }
        })
    }
}

/// Render a zone measurement entity.
fn zone_measurement(
    instance: &str,
    zone_id: &str,
    metric: &str,
    value: f32,
    unit: &str,
) -> EntityState {
    EntityState {
        unique_id: format!("aquila:{instance}:{zone_id}:{metric}"),
        entity_id: format!("sensor.aquila_{zone_id}_{metric}"),
        kind: EntityKind::Sensor,
        state: value.to_string(),
        unit: Some(unit.to_string()),
        device_class: Some("measurement".to_string()),
        entity_category: None,
    }
}

/// The HA `on`/`off` string for a boolean.
const fn on_off(value: bool) -> &'static str {
    if value {
        "on"
    } else {
        "off"
    }
}

/// The HA object-id fragment for a zone: HA uses underscores, slugs use
/// hyphens.
#[must_use]
pub fn ha_slug(zone: &ZoneId) -> String {
    zone.as_str().replace('-', "_")
}
