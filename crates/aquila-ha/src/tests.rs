//! `aquila-ha` tests: the projection is exercised through the public interface.

use aquila_geometry::Topology;
use aquila_sensing::{Sink, SinkError};
use aquila_types::{
    DeviceType, Event, EventKind, EventPayload, EventScope, FirmwareRef, Home, MacAddr, Node,
    NodeId, Slug, Witness, Zone, ZoneId, CONTRACT_VERSION,
};

use crate::{EntityKind, EntityModel, HaSink};

fn slug(value: &str) -> Slug {
    Slug(value.to_string())
}

/// A minimal one-zone, one-node contract topology.
fn topology() -> Topology {
    let home = slug("home");
    let zone = slug("living-room");
    let firmware = FirmwareRef {
        version: "test".to_string(),
        sha256: None,
    };
    Topology::load(aquila_types::Topology {
        home: Home {
            id: home.clone(),
            name: "Home".to_string(),
            time_zone: "Australia/Sydney".to_string(),
            contract: CONTRACT_VERSION,
        },
        zones: vec![aquila_types::Zone {
            id: zone.clone(),
            origin_home_m: [0.0, 0.0, 0.0],
            extent_home_m: [5.0, 5.0, 5.0],
            name: "Living".to_string(),
        }],
        nodes: vec![Node {
            id: NodeId(1),
            zone: zone.clone(),
            name: "node-1".to_string(),
            mac: MacAddr([0, 0, 0, 0, 0, 1]),
            device: DeviceType::Esp32S3,
            firmware,
            pos_home_m: [0.0, 0.0, 2.4],
            installed_unix_ms: 0,
        }],
        pairs: vec![],
    })
    .expect("valid topology")
}

fn event(scope: EventScope, kind: EventKind, payload: EventPayload) -> Event {
    Event {
        id: "evt-1".to_string(),
        home: slug("home"),
        t_unix_ms: 1_000,
        dedup_key: "home:test".to_string(),
        scope,
        kind,
        payload,
        witness: Witness::Measured,
    }
}

#[test]
fn topology_seeds_one_zone_per_room() {
    let model = EntityModel::from_topology(&topology(), "inst");
    assert_eq!(model.zones().len(), 1);
    assert!(model.zone(&slug("living-room")).is_some());
    assert_eq!(model.home().as_str(), "home");
}

#[test]
fn events_project_to_ha_entities() {
    let room = slug("living-room");
    let mut model = EntityModel::from_topology(&topology(), "inst");

    model.apply(&event(
        EventScope::Zone(room.clone()),
        EventKind::PresenceChanged,
        EventPayload::Presence {
            occupied: true,
            probability: 0.93,
        },
    ));
    model.apply(&event(
        EventScope::Zone(room.clone()),
        EventKind::VitalAlert,
        EventPayload::Vitals {
            metric: "breathing_rate".to_string(),
            value: 14.5,
            unit: "brpm".to_string(),
        },
    ));
    model.apply(&event(
        EventScope::Home,
        EventKind::OccupancyChanged,
        EventPayload::Occupancy {
            by_zone: vec![(room.clone(), 1)],
        },
    ));

    let zones = model.zones();
    let zone = zones.get(&room).expect("zone projected");
    assert_eq!(zone.present, Some(true));
    assert_eq!(zone.breathing_brpm, Some(14.5));
    assert_eq!(zone.count, Some(1));
    assert_eq!(model.home_count(), Some(1));
    assert_eq!(model.home_occupancy(), Some(true));

    let entities = model.entities();
    let presence = entities
        .iter()
        .find(|e| e.entity_id == "binary_sensor.aquila_living_room_presence")
        .expect("presence entity");
    assert_eq!(presence.kind, EntityKind::BinarySensor);
    assert_eq!(presence.state, "on");
    assert_eq!(presence.device_class.as_deref(), Some("presence"));

    let breathing = entities
        .iter()
        .find(|e| e.entity_id == "sensor.aquila_living_room_breathing_rate")
        .expect("breathing entity");
    assert_eq!(breathing.unit.as_deref(), Some("brpm"));

    assert!(entities
        .iter()
        .any(|e| e.entity_id == "binary_sensor.aquila_occupancy" && e.state == "on"));
    assert!(entities
        .iter()
        .any(|e| e.entity_id == "sensor.aquila_person_count" && e.state == "1"));
}

#[test]
fn the_sink_updates_a_shared_model() {
    let room = slug("living-room");
    let sink = HaSink::new(EntityModel::from_topology(&topology(), "inst"));
    let handle = sink.handle();
    let mut sink = sink;

    let err: Result<(), SinkError> = sink.emit(&event(
        EventScope::Zone(room.clone()),
        EventKind::PresenceChanged,
        EventPayload::Presence {
            occupied: false,
            probability: 0.01,
        },
    ));
    assert!(err.is_ok());

    let model = handle.lock().expect("not poisoned");
    assert_eq!(model.zone(&room).and_then(|z| z.present), Some(false));
}

#[test]
fn snapshot_is_json_with_zones_and_health() {
    let zone: ZoneId = slug("living-room");
    let mut model = EntityModel::from_topology(&topology(), "inst");
    model.apply(&event(
        EventScope::Zone(zone),
        EventKind::CountChanged,
        EventPayload::Count { people: 2 },
    ));
    let snapshot = model.snapshot();
    assert_eq!(snapshot["home"], "home");
    assert_eq!(snapshot["zones"]["living-room"]["person_count"], 2);
    assert_eq!(snapshot["health"]["core_connected"], true);
}

#[test]
fn unknown_events_are_ignored_not_guessed() {
    let mut model = EntityModel::new(slug("home"), "inst");
    model.apply(&event(
        EventScope::Home,
        EventKind::ModelChanged,
        EventPayload::Change {
            summary: "noise".to_string(),
        },
    ));
    assert!(model.entities().is_empty());
}
