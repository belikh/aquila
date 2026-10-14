//! Unit and property tests for `aquila-geometry`.
//!
//! These pin the behaviours the register promises: topology-query correctness,
//! referential integrity at load, the confidence behaviour of
//! `estimate_position`, the confidence algebra, frame round-trips, and two
//! proptest invariants (attribution totality for nodes in the topology, and
//! pair-scope symmetry).

use proptest::prelude::*;

use aquila_types::{Slug, CONTRACT_VERSION};

use crate::*;

fn slug(value: &str) -> Slug {
    Slug::new(value).expect("valid slug")
}

fn node_at(id: u16, room: &str, pos: Option<[f32; 3]>) -> Node {
    Node {
        id: NodeId(id),
        room: slug(room),
        name: format!("n{id:02}"),
        mac: MacAddr([0, id as u8, 0, 0, 0, 0]),
        device: DeviceType::Esp32S3,
        firmware: FirmwareRef {
            version: "0.4.1".to_string(),
            sha256: None,
        },
        pos_room_m: pos,
        installed_unix_ms: 0,
    }
}

fn pair(tx: u16, rx: u16, scope: PairScope) -> NodePair {
    NodePair {
        id: slug(&format!("{}-{}", tx, rx)),
        home: slug("home-1"),
        tx: NodeId(tx),
        rx: NodeId(rx),
        scope,
    }
}

/// A small, valid home: two floors, three rooms, six nodes (one unsurveyed) and
/// three ordered pairs spanning every scope.
fn sample_topology() -> aquila_types::Topology {
    aquila_types::Topology {
        home: Home {
            id: slug("home-1"),
            name: "Test home".to_string(),
            time_zone: "Australia/Sydney".to_string(),
            contract: CONTRACT_VERSION,
        },
        floors: vec![
            Floor {
                id: slug("F0"),
                home: slug("home-1"),
                name: "Ground".to_string(),
                level: 0,
                elevation_m: 0.0,
            },
            Floor {
                id: slug("F1"),
                home: slug("home-1"),
                name: "First".to_string(),
                level: 1,
                elevation_m: 3.0,
            },
        ],
        rooms: vec![
            Room {
                id: slug("R0"),
                floor: slug("F0"),
                name: "Living".to_string(),
                kind: RoomKind::Living,
                origin_home_m: [0.0, 0.0, 0.0],
                yaw_deg: 0.0,
                area_m2: Some(20.0),
            },
            Room {
                id: slug("R1"),
                floor: slug("F0"),
                name: "Kitchen".to_string(),
                kind: RoomKind::Kitchen,
                origin_home_m: [5.0, 0.0, 0.0],
                yaw_deg: 0.0,
                area_m2: Some(10.0),
            },
            Room {
                id: slug("R2"),
                floor: slug("F1"),
                name: "Bed".to_string(),
                kind: RoomKind::Bedroom,
                origin_home_m: [0.0, 0.0, 3.0],
                yaw_deg: 90.0,
                area_m2: Some(12.0),
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
        pairs: vec![
            pair(1, 2, PairScope::Room),
            pair(1, 3, PairScope::SameFloor),
            pair(1, 4, PairScope::CrossFloor),
        ],
    }
}

fn loaded() -> Topology {
    Topology::load(sample_topology()).expect("sample topology is valid")
}

fn name(room: &RoomId) -> &str {
    room.as_str()
}

#[test]
fn attribution_room_zone_and_floor() {
    let t = loaded();

    assert_eq!(t.room_of(NodeId(1)).value.map(name), Some("R0"));
    assert_eq!(t.room_of(NodeId(1)).confidence, Confidence::ONE);
    assert_eq!(t.room_of(NodeId(4)).value.map(name), Some("R2"));

    // In v1 a zone is a room.
    assert_eq!(t.zone_of(NodeId(3)).value.flatten().map(name), Some("R1"));

    assert_eq!(t.floor_of(NodeId(1)).value.map(name), Some("F0"));
    assert_eq!(t.floor_of(NodeId(4)).value.map(name), Some("F1"));
}

#[test]
fn attribution_is_honest_for_unknown_nodes() {
    let t = loaded();
    let unknown = NodeId(999);
    assert!(t.room_of(unknown).value.is_none());
    assert_eq!(t.room_of(unknown).confidence, Confidence::ZERO);
    assert!(t.zone_of(unknown).value.is_none());
    assert!(t.floor_of(unknown).value.is_none());
    assert!(t.position(unknown).value.is_none());
}

#[test]
fn load_rejects_broken_referential_integrity() {
    let mut model = sample_topology();
    model
        .nodes
        .push(node_at(7, "missing-room", Some([0.0, 0.0, 0.0])));
    assert!(matches!(
        Topology::load(model),
        Err(GeometryError::InvalidTopology(_))
    ));
}

#[test]
fn load_rejects_scope_that_disagrees_with_topology() {
    let mut model = sample_topology();
    // n1 and n2 share room R0; claiming CrossFloor must be rejected.
    model.pairs[0].scope = PairScope::CrossFloor;
    assert!(matches!(
        Topology::load(model),
        Err(GeometryError::InvalidTopology(_))
    ));
}

#[test]
fn scope_is_derived_and_symmetric() {
    let t = loaded();
    assert_eq!(t.scope_of(NodeId(1), NodeId(2)), Some(PairScope::Room));
    assert_eq!(t.scope_of(NodeId(1), NodeId(3)), Some(PairScope::SameFloor));
    assert_eq!(
        t.scope_of(NodeId(1), NodeId(4)),
        Some(PairScope::CrossFloor)
    );
    assert_eq!(
        t.scope_of(NodeId(1), NodeId(2)),
        t.scope_of(NodeId(2), NodeId(1))
    );
    assert_eq!(t.scope_of(NodeId(1), NodeId(999)), None);

    assert_eq!(scope_label(PairScope::Room), "intra_room");
    assert_eq!(scope_label(PairScope::SameFloor), "inter_room");
    assert_eq!(scope_label(PairScope::CrossFloor), "inter_floor");
}

#[test]
fn position_reports_survey_and_unknown() {
    let t = loaded();
    assert_eq!(t.position(NodeId(1)).value, Some([1.0, 1.0, 0.0]));
    assert_eq!(t.position(NodeId(1)).confidence, Confidence::ONE);
    assert!(t.position(NodeId(5)).value.is_none());
}

#[test]
fn canonical_pair_id_is_orientation_free() {
    assert_eq!(
        Topology::canonical_pair_id(NodeId(12), NodeId(7)),
        Topology::canonical_pair_id(NodeId(7), NodeId(12))
    );
    assert_eq!(Topology::canonical_pair_id(NodeId(7), NodeId(12)), "7-12");
}

#[test]
fn room_frame_round_trips() {
    let t = loaded();
    let frame = t.room_frame(&slug("R2")).expect("room exists");
    let p = [1.3_f32, -0.7];
    let back = frame.apply_inverse(frame.apply(p));
    assert!((back[0] - p[0]).abs() < 1e-4);
    assert!((back[1] - p[1]).abs() < 1e-4);

    // Composing a frame with its inverse is the identity.
    let identity = frame.compose(&frame.inverse());
    let q = identity.apply(p);
    assert!((q[0] - p[0]).abs() < 1e-4);
    assert!((q[1] - p[1]).abs() < 1e-4);
}

#[test]
fn node_frame_is_unknown_without_a_survey() {
    let t = loaded();
    assert!(t.node_frame(NodeId(1)).is_known());
    assert!(!t.node_frame(NodeId(5)).is_known());
    assert_eq!(t.node_frame(NodeId(5)).confidence, Confidence::ZERO);
}

#[test]
fn estimate_position_is_honest_without_evidence() {
    let t = loaded();
    assert!(t.estimate_position(&[]).value.is_none());
    assert_eq!(t.estimate_position(&[]).confidence, Confidence::ZERO);

    // Links that name nodes absent from the topology cannot be attributed.
    let ghosts = [LinkObservation::new(NodeId(999), NodeId(998), -50.0)];
    assert!(t.estimate_position(&ghosts).value.is_none());
}

#[test]
fn estimate_position_uses_proximity_as_primary() {
    let t = loaded();
    let links = [LinkObservation::new(NodeId(1), NodeId(2), -50.0)];
    let scored = t.estimate_position(&links);
    let estimate = scored.value.expect("an estimate");
    assert_eq!(estimate.method, PositionMethod::ProximityFingerprint);
    assert_eq!(estimate.resolution, PositionResolution::Room);
    assert_eq!(estimate.room.as_ref().map(name), Some("R0"));
    assert!(estimate.advisory);
    assert!(estimate.position_home_m.is_none());
    assert!(scored.confidence.get() > 0.0);
}

#[test]
fn estimate_position_trilaterates_when_anchors_allow() {
    let t = loaded();
    let links = [
        LinkObservation::new(NodeId(1), NodeId(2), -45.0),
        LinkObservation::new(NodeId(1), NodeId(3), -55.0),
        LinkObservation::new(NodeId(2), NodeId(3), -50.0),
    ];
    let estimate = t.estimate_position(&links).value.expect("an estimate");
    assert_eq!(estimate.method, PositionMethod::TrilaterationSecondary);
    assert!(estimate.position_home_m.is_some());
    assert!(estimate.position_room_m.is_some());
    assert!(estimate.error_ellipse_95_m.is_some());
    assert!(estimate.advisory);
}

#[test]
fn estimate_position_demotes_when_ellipse_exceeds_the_room() {
    // A room so small that any honest ellipse overflows it forces room-level.
    let model = aquila_types::Topology {
        home: Home {
            id: slug("home-1"),
            name: "Tiny".to_string(),
            time_zone: "Australia/Sydney".to_string(),
            contract: CONTRACT_VERSION,
        },
        floors: vec![Floor {
            id: slug("F0"),
            home: slug("home-1"),
            name: "Ground".to_string(),
            level: 0,
            elevation_m: 0.0,
        }],
        rooms: vec![Room {
            id: slug("Rt"),
            floor: slug("F0"),
            name: "Cupboard".to_string(),
            kind: RoomKind::Utility,
            origin_home_m: [0.0, 0.0, 0.0],
            yaw_deg: 0.0,
            area_m2: Some(0.0001),
        }],
        nodes: vec![
            node_at(1, "Rt", Some([1.0, 1.0, 0.0])),
            node_at(2, "Rt", Some([3.0, 1.0, 0.0])),
            node_at(3, "Rt", Some([6.0, 3.0, 0.0])),
        ],
        pairs: vec![
            pair(1, 2, PairScope::Room),
            pair(1, 3, PairScope::Room),
            pair(2, 3, PairScope::Room),
        ],
    };
    let t = Topology::load(model).expect("valid");
    let links = [
        LinkObservation::new(NodeId(1), NodeId(2), -45.0),
        LinkObservation::new(NodeId(1), NodeId(3), -55.0),
        LinkObservation::new(NodeId(2), NodeId(3), -50.0),
    ];
    let estimate = t.estimate_position(&links).value.expect("an estimate");
    let ellipse = estimate.error_ellipse_95_m.expect("ellipse");
    assert!(
        ellipse.major_m > 0.01,
        "test geometry should overflow the cupboard"
    );
    assert_eq!(estimate.resolution, PositionResolution::Room);
}

#[test]
fn estimate_position_confidence_is_monotone_in_evidence() {
    let t = loaded();
    let one = t.estimate_position(&[LinkObservation::new(NodeId(1), NodeId(2), -60.0)]);
    let two = t.estimate_position(&[
        LinkObservation::new(NodeId(1), NodeId(2), -60.0),
        LinkObservation::new(NodeId(1), NodeId(5), -60.0),
    ]);
    assert!(two.confidence.get() >= one.confidence.get());
}

#[test]
fn estimate_position_reports_ambiguity() {
    let t = loaded();
    // One intra-room link in R0 and one in R1, equally weighted: the top two
    // rooms tie, so the attribution must be ambiguous rather than confident.
    let links = [
        LinkObservation::new(NodeId(1), NodeId(2), -60.0),
        LinkObservation::new(NodeId(3), NodeId(6), -60.0),
    ];
    let scored = t.estimate_position(&links);
    assert!(scored.ambiguous);
}

#[test]
fn trilateration_refuses_degenerate_geometry() {
    let model = PathLossModel::default();
    // Fewer than three anchors.
    assert!(trilaterate(&[[0.0, 0.0, -50.0], [1.0, 0.0, -50.0]], &model).is_none());
    // Collinear anchors cannot fix a plane.
    assert!(trilaterate(
        &[[0.0, 0.0, -50.0], [1.0, 0.0, -50.0], [2.0, 0.0, -50.0]],
        &model
    )
    .is_none());
}

#[test]
fn trilateration_recovers_a_known_target() {
    let model = PathLossModel::default();
    let target = [2.0_f32, 2.0_f32];
    let anchors = [[0.0_f32, 0.0_f32], [4.0, 0.0], [2.0, 4.0]];
    let samples: Vec<[f32; 3]> = anchors
        .iter()
        .map(|a| {
            let d = ((a[0] - target[0]).powi(2) + (a[1] - target[1]).powi(2)).sqrt();
            [a[0], a[1], model.rssi_for_range_m(d)]
        })
        .collect();
    let tri = trilaterate(&samples, &model).expect("locates");
    assert!((tri.position[0] - target[0]).abs() < 0.5);
    assert!((tri.position[1] - target[1]).abs() < 0.5);
    assert!(tri.error_ellipse_95.major_m.is_finite());
}

#[test]
fn path_loss_round_trips_range() {
    let model = PathLossModel::default();
    for range in [0.5_f32, 1.0, 2.0, 5.0] {
        let back = model.range_m(model.rssi_for_range_m(range));
        assert!((back - range).abs() < 1e-2, "range {range} => {back}");
    }
}

#[test]
fn confidence_algebra_is_as_specified() {
    let half = Confidence::new(0.5).expect("unit");
    let two_thirds = Confidence::new(0.666_666_7).expect("unit");
    assert!((chain(half, half).get() - 0.25).abs() < 1e-6);
    assert!((independent(half, half).get() - 0.75).abs() < 1e-6);
    assert!((conjunction(half, two_thirds).get() - 0.5).abs() < 1e-6);

    // Clamping never invents certainty and never panics on non-finite input.
    assert_eq!(unit_confidence(f32::NAN), Confidence::ZERO);
    assert_eq!(unit_confidence(2.0), Confidence::ONE);
    assert_eq!(unit_confidence(-1.0), Confidence::ZERO);
}

proptest! {
    /// Attribution is total for nodes that belong to the topology, and honest
    /// for every other identity.
    #[test]
    fn attribution_is_total_for_nodes_in_the_topology(value in any::<u16>()) {
        let t = loaded();
        let id = NodeId(value);
        let exists = t.model().nodes.iter().any(|n| n.id == id);
        let room = t.room_of(id);
        prop_assert_eq!(room.value.is_some(), exists);
        prop_assert_eq!(t.zone_of(id).value.is_some(), exists);
        prop_assert_eq!(t.floor_of(id).value.is_some(), exists);
        if exists {
            prop_assert_eq!(room.confidence, Confidence::ONE);
        } else {
            prop_assert_eq!(room.confidence, Confidence::ZERO);
        }
    }

    /// Scope is symmetric over a pair and agrees with the contract derivation.
    #[test]
    fn scope_of_is_symmetric_over_a_pair(i in 0usize..6, j in 0usize..6) {
        let t = loaded();
        let ids: Vec<NodeId> = t.model().nodes.iter().map(|n| n.id).collect();
        let a = ids[i.min(ids.len() - 1)];
        let b = ids[j.min(ids.len() - 1)];
        prop_assert_eq!(t.scope_of(a, b), t.scope_of(b, a));
        if let Some(scope) = t.scope_of(a, b) {
            prop_assert_eq!(scope, t.model().derive_scope(a, b));
        }
    }

    /// The canonical pair identity is orientation-free.
    #[test]
    fn canonical_pair_id_is_symmetric(a in any::<u16>(), b in any::<u16>()) {
        prop_assert_eq!(
            Topology::canonical_pair_id(NodeId(a), NodeId(b)),
            Topology::canonical_pair_id(NodeId(b), NodeId(a))
        );
    }

    /// Frame transforms round-trip within tolerance.
    #[test]
    fn frame_round_trip_is_stable(x in -50.0f32..50.0, y in -50.0f32..50.0) {
        let frame = Transform2::new([1.5, -2.0], 0.7, Confidence::ONE);
        let back = frame.apply_inverse(frame.apply([x, y]));
        prop_assert!((back[0] - x).abs() < 1e-3);
        prop_assert!((back[1] - y).abs() < 1e-3);
    }

    /// More evidence never lowers confidence: noisy-OR is a lower bound.
    #[test]
    fn independent_confidence_is_monotone(a in 0.0f32..=1.0, b in 0.0f32..=1.0) {
        let ca = unit_confidence(a);
        let cb = unit_confidence(b);
        let combined = independent(ca, cb);
        prop_assert!(combined.get() >= ca.get() - 1e-6);
        prop_assert!(combined.get() >= cb.get() - 1e-6);
    }

    /// Chain and conjunction never exceed either input.
    #[test]
    fn chain_and_conjunction_are_lower_bounds(a in 0.0f32..=1.0, b in 0.0f32..=1.0) {
        let ca = unit_confidence(a);
        let cb = unit_confidence(b);
        prop_assert!(chain(ca, cb).get() <= ca.get() + 1e-6);
        prop_assert!(chain(ca, cb).get() <= cb.get() + 1e-6);
        prop_assert!(conjunction(ca, cb).get() <= ca.get() + 1e-6);
        prop_assert!(conjunction(ca, cb).get() <= cb.get() + 1e-6);
    }
}
