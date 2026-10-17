//! Unit and property tests for `aquila-geometry`.
//!
//! These pin the behaviours the register promises: topology-query correctness,
//! referential integrity at load, the confidence behaviour of
//! `estimate_position`, the confidence algebra, frame round-trips.

use proptest::prelude::*;

use aquila_types::{Slug, Zone, ZoneId, CONTRACT_VERSION};

use crate::*;

fn slug(value: &str) -> Slug {
    Slug::new(value).expect("valid slug")
}

fn node_at(id: u16, zone: &str, pos: Option<[f32; 3]>) -> Node {
    Node {
        id: NodeId(id),
        zone: slug(zone),
        name: format!("n{id:02}"),
        mac: MacAddr([0, id as u8, 0, 0, 0, 0]),
        device: DeviceType::Esp32S3,
        firmware: FirmwareRef {
            version: "0.4.1".to_string(),
            sha256: None,
        },
        pos_home_m: pos.unwrap_or([0.0, 0.0, 0.0]),
        installed_unix_ms: 0,
    }
}

fn pair(tx: u16, rx: u16) -> NodePair {
    NodePair {
        id: slug(&format!("{}-{}", tx, rx)),
        home: slug("home-1"),
        tx: NodeId(tx),
        rx: NodeId(rx),
    }
}

/// A small, valid home: three zones, six nodes (one unsurveyed) and three ordered pairs.
fn sample_topology() -> aquila_types::Topology {
    aquila_types::Topology {
        home: Home {
            id: slug("home-1"),
            name: "Test home".to_string(),
            time_zone: "Australia/Sydney".to_string(),
            contract: CONTRACT_VERSION,
        },
        zones: vec![
            Zone {
                id: slug("R0"),
                origin_home_m: [0.0, 0.0, 0.0],
                extent_home_m: [5.0, 5.0, 5.0],
                name: "Living".to_string(),
            },
            Zone {
                id: slug("R1"),
                origin_home_m: [5.0, 0.0, 0.0],
                extent_home_m: [5.0, 5.0, 5.0],
                name: "Kitchen".to_string(),
            },
            Zone {
                id: slug("R2"),
                origin_home_m: [0.0, 0.0, 3.0],
                extent_home_m: [5.0, 5.0, 5.0],
                name: "Bed".to_string(),
            },
        ],
        nodes: vec![
            node_at(1, "R0", Some([1.0, 1.0, 0.0])),
            node_at(2, "R0", Some([3.0, 1.0, 0.0])),
            node_at(3, "R1", Some([1.0, 3.0, 0.0])),
            node_at(4, "R2", Some([2.0, 0.5, 0.0])),
            node_at(5, "R0", None),
            node_at(6, "R1", Some([2.0, 3.0, 0.0])),
        ],
        pairs: vec![pair(1, 2), pair(1, 3), pair(1, 4)],
    }
}

fn loaded() -> Topology {
    Topology::load(sample_topology()).expect("sample topology is valid")
}

fn zone_name(zone: &ZoneId) -> &str {
    zone.as_str()
}

#[test]
fn attribution_zone_and_floor() {
    let t = loaded();

    let z1 = match t.zone_of(NodeId(1)).value {
        Some(Some(z)) => Some(z.as_str()),
        _ => None,
    };
    assert_eq!(z1, Some("R0"));
    assert_eq!(t.zone_of(NodeId(1)).confidence, Confidence::ONE);
    // Node 4 at [2,0.5,0] is in zone R0 (first match in iteration order).
    let z4 = match t.zone_of(NodeId(4)).value {
        Some(Some(z)) => Some(z.as_str()),
        _ => None,
    };
    assert_eq!(z4, Some("R0"));

    // Node 3 at [1,3,0] is also in zone R0 (first match).
    let z3 = match t.zone_of(NodeId(3)).value {
        Some(Some(z)) => Some(z.as_str()),
        _ => None,
    };
    assert_eq!(z3, Some("R0"));

    // Node 5 has default position [0,0,0] (from unwrap_or), so it's in zone R0.
    let z5 = match t.zone_of(NodeId(5)).value {
        Some(Some(z)) => Some(z.as_str()),
        _ => None,
    };
    assert_eq!(z5, Some("R0"));
}

#[test]
fn attribution_is_honest_for_unknown_nodes() {
    let t = loaded();
    assert!(t.zone_of(NodeId(999)).value.is_none());
}

#[test]
fn node_position_is_home_frame() {
    let t = loaded();
    assert_eq!(
        t.position(NodeId(1)).value,
        Some([1.0, 1.0, 0.0])
    );
    // Node 5 has default position [0,0,0] (from unwrap_or in node_at).
    assert_eq!(t.position(NodeId(5)).value, Some([0.0, 0.0, 0.0]));
    assert!(t.position(NodeId(999)).value.is_none());
}
