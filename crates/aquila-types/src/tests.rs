//! Unit tests: serde round-trips for every record, invariant validation, and
//! the contract version.
//!
//! These tests are the pin on the contract. A change to a shape that breaks a
//! round-trip, or to an invariant that stops rejecting a malformed value, fails
//! here first.

use std::net::SocketAddr;

use crate::*;

/// Serialise, deserialise and assert the value survives unchanged, and that the
/// serialised form is byte-stable.
fn round_trip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + core::fmt::Debug,
{
    let json = serde_json::to_string(value).expect("serialise");
    let again = serde_json::to_string(value).expect("serialise again");
    assert_eq!(json, again, "serialisation is not byte-stable: {json}");
    let back: T = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(&back, value, "round trip mismatch; json = {json}");
}

fn slug(value: &str) -> Slug {
    Slug::new(value).expect("valid slug")
}

fn node(id: u16) -> NodeId {
    NodeId(id)
}

fn span() -> TimeSpan {
    TimeSpan {
        start_mono_ns: 1_000,
        end_mono_ns: 2_000,
        start_unix_ms: 1_700_000_000_000,
        end_unix_ms: 1_700_000_001_000,
    }
}

fn csi_frame() -> CsiFrame {
    CsiFrame {
        node_id: node(1),
        boot_epoch: 3,
        sequence: 42,
        device_time_us: 123_456,
        band: Band::B2G4,
        channel: 6,
        bandwidth_mhz: 20,
        subcarrier_offset: -32,
        i_q: vec![0, 0, 2, 1, 2, -1],
        rssi_dbm: -40,
        noise_floor_dbm: -90,
        phy: PhyFlags(PhyFlags::HT | PhyFlags::SHORT_GI),
        first_word_invalid: false,
    }
}

fn envelope() -> CsiEnvelope {
    CsiEnvelope {
        frame: csi_frame(),
        host_rx_mono_ns: 555,
        host_rx_unix_ms: 1_700_000_000_000,
        source_addr: Some(SocketAddr::from(([192, 0, 2, 1], 9100))),
    }
}

const KEYPOINT_IDS: [KeypointId; KEYPOINT_COUNT] = [
    KeypointId::Nose,
    KeypointId::LeftEye,
    KeypointId::RightEye,
    KeypointId::LeftEar,
    KeypointId::RightEar,
    KeypointId::LeftShoulder,
    KeypointId::RightShoulder,
    KeypointId::LeftElbow,
    KeypointId::RightElbow,
    KeypointId::LeftWrist,
    KeypointId::RightWrist,
    KeypointId::LeftHip,
    KeypointId::RightHip,
    KeypointId::LeftKnee,
    KeypointId::RightKnee,
    KeypointId::LeftAnkle,
    KeypointId::RightAnkle,
];

fn pose_keys() -> [Keypoint; KEYPOINT_COUNT] {
    core::array::from_fn(|i| Keypoint {
        id: KEYPOINT_IDS[i],
        x_m: i as f32,
        y_m: 0.5,
        z_m: None,
        confidence: 0.5,
    })
}

fn home() -> Home {
    Home {
        id: slug("home-01"),
        name: "Home".to_string(),
        time_zone: "Australia/Sydney".to_string(),
        contract: CONTRACT_VERSION,
    }
}

fn floor() -> Floor {
    Floor {
        id: slug("ground-floor"),
        home: slug("home-01"),
        name: "Ground".to_string(),
        level: 0,
        elevation_m: 0.0,
    }
}

fn room() -> Room {
    Room {
        id: slug("kitchen-01"),
        floor: slug("ground-floor"),
        name: "Kitchen".to_string(),
        kind: RoomKind::Kitchen,
        origin_home_m: [0.0, 0.0, 0.0],
        yaw_deg: 0.0,
        area_m2: Some(12.5),
    }
}

fn topology_node(id: u16, room_id: &str) -> Node {
    Node {
        id: node(id),
        room: slug(room_id),
        name: format!("node-{id}"),
        mac: MacAddr([0x02, 0, 0, 0, 0, id as u8]),
        device: DeviceType::Esp32S3,
        firmware: FirmwareRef {
            version: "1.0.0".to_string(),
            sha256: Some([7u8; 32]),
        },
        pos_room_m: Some([1.0, 2.0, 2.5]),
        installed_unix_ms: 1_700_000_000_000,
    }
}

fn topology() -> Topology {
    Topology {
        home: home(),
        floors: vec![floor()],
        rooms: vec![room()],
        nodes: vec![
            topology_node(1, "kitchen-01"),
            topology_node(2, "kitchen-01"),
        ],
        pairs: vec![NodePair {
            id: slug("pair-1-2"),
            home: slug("home-01"),
            tx: node(1),
            rx: node(2),
            scope: PairScope::Room,
        }],
    }
}

fn session() -> Session {
    Session {
        id: slug("session-01"),
        name: "Evening".to_string(),
        home: slug("home-01"),
        nodes: vec![node(1), node(2)],
        started_unix_ms: 1_700_000_000_000,
        ended_unix_ms: Some(1_700_000_060_000),
        contract: CONTRACT_VERSION,
        build: BuildId {
            git_sha: "38001ab".to_string(),
            dirty: false,
        },
        config_hash: Some([1u8; 32]),
        seed: 7,
        artifacts: vec![SessionArtifact {
            path: "stream.bin".to_string(),
            sha256: [2u8; 32],
            frames: 100,
        }],
        ground_truth: Some(GroundTruthRef {
            path: "ground_truth.jsonl".to_string(),
            sha256: Some([3u8; 32]),
            kind: GtKind::Manual,
        }),
        notes: "fixture".to_string(),
    }
}

fn env_fingerprint() -> EnvFingerprint {
    EnvFingerprint {
        host: "ci".to_string(),
        rustc: "1.98.1".to_string(),
        os: "linux".to_string(),
        thread_count: 8,
        rng: "splitmix64:7".to_string(),
    }
}

fn dataset() -> DatasetRef {
    DatasetRef {
        name: "golden".to_string(),
        split: Split::InDomain,
    }
}

fn ledger_record(kind: MetricKind) -> LedgerRecord {
    LedgerRecord {
        id: "abc123".to_string(),
        commit: BuildId {
            git_sha: "38001ab".to_string(),
            dirty: false,
        },
        contract: CONTRACT_VERSION,
        session: slug("session-01"),
        metric: slug("accuracy.presence_f1"),
        kind,
        value: 0.91,
        unit: "F1".to_string(),
        samples: 200,
        ci: Some(Interval {
            low: 0.88,
            high: 0.94,
        }),
        dataset: dataset(),
        model: Some(ModelRef {
            id: "presence-v1".to_string(),
            sha256: [4u8; 32],
            onnx_opset: 17,
        }),
        config_hash: [5u8; 32],
        threshold: Some(0.9),
        passed: Some(true),
        env: env_fingerprint(),
        created_unix_ms: 1_700_000_000_000,
        supersedes: None,
    }
}

fn calibration() -> Calibration {
    Calibration {
        home: slug("home-01"),
        contract: CONTRACT_VERSION,
        created_unix_ms: 1_700_000_000_000,
        session: slug("session-01"),
        topology_hash: [6u8; 32],
        node_positions_m: vec![(node(1), [1.0, 2.0, 2.5]), (node(2), [3.0, 4.0, 2.5])],
        node_offsets_db: vec![(node(1), 0.5), (node(2), -0.5)],
        pair_offsets: vec![(
            slug("pair-1-2"),
            PairCorrection {
                amplitude_db: 0.25,
                phase_rad: 0.1,
            },
        )],
        validity: span(),
        witness: Witness::Measured,
    }
}

fn baseline() -> Baseline {
    Baseline {
        home: slug("home-01"),
        session: slug("session-01"),
        scope: BaselineScope::Zone(slug("kitchen-01")),
        span: span(),
        amplitude_ref_db: vec![0.0, 1.0, 2.0],
        phase_ref_rad: vec![0.0, 0.1, 0.2],
        created_unix_ms: 1_700_000_000_000,
        witness: Witness::Measured,
    }
}

fn vitals_reading() -> VitalsReading {
    VitalsReading {
        subject: Some(SubjectId(1)),
        room: slug("kitchen-01"),
        span: span(),
        breathing: Some(Estimate {
            value: 15.0,
            confidence: 0.8,
        }),
        heart_rate: Some(Estimate {
            value: 70.0,
            confidence: 0.7,
        }),
        status: VitalsStatus::Ok,
        witness: Witness::Measured,
    }
}

fn detection() -> Detection {
    Detection {
        scope: DetectionScope::Room(slug("kitchen-01")),
        span: span(),
        kind: DetectionKind::Presence,
        confidence: 0.9,
        value: DetectionValue::Probability(0.9),
        contributors: vec![node(1), node(2)],
        witness: Witness::Measured,
    }
}

fn event() -> Event {
    Event {
        id: "evt-123".to_string(),
        home: slug("home-01"),
        t_unix_ms: 1_700_000_000_000,
        dedup_key: "home-01:presence_changed:kitchen-01".to_string(),
        scope: EventScope::Room(slug("kitchen-01")),
        kind: EventKind::PresenceChanged,
        payload: EventPayload::Presence {
            occupied: true,
            probability: 0.9,
        },
        witness: Witness::Measured,
    }
}

fn clock_model() -> ClockModel {
    ClockModel {
        node: node(1),
        boot_epoch: 3,
        offset_ns: -1_234_567,
        drift_ppm: 12.5,
        uncertainty_ns: 500_000,
        estimated_mono_ns: 9_000_000,
        samples: 64,
        witness: Witness::Measured,
    }
}

#[test]
fn version_constant_and_display() {
    assert_eq!(CONTRACT_VERSION.major, 1);
    assert_eq!(CONTRACT_VERSION.minor, 0);
    assert_eq!(CONTRACT_VERSION_MAJOR, 1);
    assert_eq!(CONTRACT_VERSION_MINOR, 0);
    assert_eq!(CONTRACT_VERSION.to_string(), "1.0");
    assert_eq!(ContractVersion::new(1, 0), CONTRACT_VERSION);
    assert!(ContractVersion::new(1, 0).is_compatible_with(&ContractVersion::new(1, 5)));
    assert!(!ContractVersion::new(1, 0).is_compatible_with(&ContractVersion::new(2, 0)));
}

#[test]
fn every_record_round_trips() {
    round_trip(&CONTRACT_VERSION);
    round_trip(&node(2));
    round_trip(&MacAddr([1, 2, 3, 4, 5, 6]));
    round_trip(&slug("kitchen-01"));
    round_trip(&SubjectId(3));
    round_trip(&CoreTime::from_nanos(42));
    round_trip(&DeviceTime::from_nanos(42));
    round_trip(&span());
    round_trip(&TimeWindow::new(CoreTime(10), CoreTime(20)).expect("window"));
    round_trip(&Band::B5G);
    round_trip(&SecondaryChannel::Above);
    round_trip(&PhyFlags(PhyFlags::LDPC));
    round_trip(&Witness::DataGated);
    round_trip(&Confidence::new(0.5).expect("confidence"));
    round_trip(&Quality::new(0.25).expect("quality"));
    round_trip(&csi_frame());
    round_trip(&envelope());
    round_trip(&SensingFrame {
        scope: SensingScope::Pair(slug("pair-1-2")),
        span: span(),
        grid: ClockGrid {
            origin_mono_ns: 1_000,
            interval_ns: 500,
            slots: 10,
        },
        frames: vec![envelope()],
    });
    round_trip(&home());
    round_trip(&floor());
    round_trip(&room());
    round_trip(&topology_node(1, "kitchen-01"));
    round_trip(&topology());
    round_trip(&Pose {
        subject: SubjectId(1),
        room: slug("kitchen-01"),
        t_unix_ms: 1_700_000_000_000,
        keys: pose_keys(),
        score: 0.75,
        witness: Witness::Measured,
    });
    round_trip(&vitals_reading());
    round_trip(&detection());
    round_trip(&event());
    round_trip(&session());
    round_trip(&SessionManifest {
        contract: CONTRACT_VERSION,
        codec: "postcard-1".to_string(),
        frames: 100,
        files: session().artifacts,
    });
    round_trip(&GroundTruthRecord {
        session: slug("session-01"),
        t_unix_ms: 1_700_000_000_000,
        kind: GtKind::CameraResearch,
        scope: EventScope::Room(slug("kitchen-01")),
        payload: EventPayload::Count { people: 2 },
    });
    round_trip(&ledger_record(MetricKind::Gated));
    round_trip(&calibration());
    round_trip(&baseline());
    round_trip(&clock_model());
}

#[test]
fn unknown_enum_variants_fall_back() {
    let band: Band = serde_json::from_str("\"6ghz\"").expect("band");
    assert_eq!(band, Band::Unknown);
    let status: VitalsStatus = serde_json::from_str("\"something\"").expect("status");
    assert_eq!(status, VitalsStatus::Unknown);
    let kind: EventKind = serde_json::from_str("\"brand_new\"").expect("kind");
    assert_eq!(kind, EventKind::Unknown);
}

#[test]
fn witness_ordering_and_value_permission() {
    assert!(Witness::Gated < Witness::Measured);
    assert!(Witness::Measured < Witness::Claimed);
    assert!(Witness::Claimed < Witness::DataGated);
    assert!(Witness::DataGated < Witness::HardwareGated);
    assert!(Witness::Gated.may_carry_value());
    assert!(Witness::Measured.may_carry_value());
    assert!(!Witness::Claimed.may_carry_value());
    assert!(!Witness::DataGated.may_carry_value());
    assert!(!Witness::HardwareGated.may_carry_value());
}

#[test]
fn identifiers_reject_emptiness() {
    assert!(Slug::new("").is_err());
    assert!(Slug::new("ok").is_ok());
    assert!(Slug(String::new()).validate().is_err());
    assert!(NodeId::UNPROVISIONED.validate_provisioned().is_err());
    assert!(node(1).validate_provisioned().is_ok());
}

#[test]
fn confidence_and_quality_ranges() {
    assert!(Confidence::new(0.0).is_ok());
    assert!(Confidence::new(1.0).is_ok());
    assert!(Confidence::new(1.5).is_err());
    assert!(Confidence::new(-0.1).is_err());
    assert!(Confidence::new(f32::NAN).is_err());
    assert!(Quality::new(0.5).is_ok());
    assert!(Quality::new(2.0).is_err());
    assert!(NodeQuality::new(0.0).is_ok());
    assert!(NodeQuality::new(1.0).is_ok());
    assert!(NodeQuality::new(1.5).is_err());
    assert!(NodeQuality::new(-0.1).is_err());
    assert!(NodeQuality::new(f32::NAN).is_err());
    assert_eq!(NodeQuality::ZERO.get(), 0.0);
    assert_eq!(NodeQuality::ONE.get(), 1.0);
    assert!(NodeQuality::new(0.3).unwrap().validate().is_ok());
}

#[test]
fn time_span_and_window_invariants() {
    assert!(span().validate().is_ok());
    let bad = TimeSpan {
        end_mono_ns: 1_000,
        ..span()
    };
    assert!(bad.validate().is_err());
    assert!(TimeWindow::new(CoreTime(5), CoreTime(10)).is_ok());
    assert!(TimeWindow::new(CoreTime(10), CoreTime(10)).is_err());
    let window = TimeWindow::new(CoreTime(5), CoreTime(10)).expect("window");
    assert!(window.contains(CoreTime(5)));
    assert!(!window.contains(CoreTime(10)));
    assert_eq!(window.duration_ns(), 5);
}

#[test]
fn csi_frame_invariants() {
    assert!(csi_frame().validate().is_ok());
    assert_eq!(csi_frame().n_subcarriers(), 3);

    // An odd-length interleaved sample is not whole I/Q pairs.
    let mut odd = csi_frame();
    odd.i_q.pop();
    assert!(odd.validate().is_err());

    // An empty sample carries no subcarriers.
    let mut empty = csi_frame();
    empty.i_q.clear();
    assert!(empty.validate().is_err());

    // The derived amplitude and phase are finite for every int8 sample.
    let frame = csi_frame();
    assert_eq!(frame.amplitude().len(), 3);
    assert_eq!(frame.phase().len(), 3);
    assert!(frame.amplitude().iter().all(|a| a.is_finite()));
    assert!(frame.phase().iter().all(|p| p.is_finite()));
    // The sample is imaginary-first: subcarrier 0 is (I, Q) = (0, 0), so its
    // amplitude is 0, and subcarrier 1 is (I, Q) = (1, 2).
    assert!((frame.amplitude_at(0)).abs() < 1e-6);
    assert!((frame.amplitude_at(1) - 5.0_f32.sqrt() / 64.0).abs() < 1e-6);
}

#[test]
fn sensing_frame_scope_invariant() {
    let frame = SensingFrame {
        scope: SensingScope::Node(node(1)),
        span: span(),
        grid: ClockGrid {
            origin_mono_ns: 0,
            interval_ns: 1,
            slots: 1,
        },
        frames: vec![envelope()],
    };
    assert!(frame.validate().is_ok());

    let mut wrong_node = frame.clone();
    wrong_node.frames[0].frame.node_id = node(2);
    assert!(wrong_node.validate().is_err());

    let mut zero_interval = frame;
    zero_interval.grid.interval_ns = 0;
    assert!(zero_interval.validate().is_err());
}

#[test]
fn pose_invariants() {
    let valid = Pose {
        subject: SubjectId(1),
        room: slug("kitchen-01"),
        t_unix_ms: 0,
        keys: pose_keys(),
        score: 0.5,
        witness: Witness::Measured,
    };
    assert!(valid.validate().is_ok());

    let mut misordered = valid.clone();
    misordered.keys[0].id = KeypointId::RightAnkle;
    assert!(misordered.validate().is_err());

    let mut bad_score = valid.clone();
    bad_score.score = 1.2;
    assert!(bad_score.validate().is_err());

    let mut bad_confidence = valid;
    bad_confidence.keys[0].confidence = -0.1;
    assert!(bad_confidence.validate().is_err());
}

#[test]
fn vitals_status_coupling() {
    assert!(vitals_reading().validate().is_ok());

    let mut no_estimates = vitals_reading();
    no_estimates.breathing = None;
    no_estimates.heart_rate = None;
    assert!(no_estimates.validate().is_err());

    let mut blocked_with_value = vitals_reading();
    blocked_with_value.status = VitalsStatus::NoSubject;
    assert!(blocked_with_value.validate().is_err());

    let mut out_of_range = vitals_reading();
    out_of_range.breathing = Some(Estimate {
        value: 100.0,
        confidence: 0.5,
    });
    assert!(out_of_range.validate().is_err());
}

#[test]
fn detection_value_kind_scope_coupling() {
    assert!(detection().validate().is_ok());

    let mut no_contributors = detection();
    no_contributors.contributors.clear();
    assert!(no_contributors.validate().is_err());

    let mut wrong_value = detection();
    wrong_value.value = DetectionValue::Count(1);
    assert!(wrong_value.validate().is_err());

    let mut bad_probability = detection();
    bad_probability.value = DetectionValue::Probability(1.5);
    assert!(bad_probability.validate().is_err());

    let mut wrong_scope = detection();
    wrong_scope.kind = DetectionKind::Occupancy;
    wrong_scope.value = DetectionValue::Occupancy(vec![(slug("kitchen-01"), 1)]);
    wrong_scope.scope = DetectionScope::Room(slug("kitchen-01"));
    assert!(wrong_scope.validate().is_err());

    let mut too_many = detection();
    too_many.kind = DetectionKind::Count;
    too_many.value = DetectionValue::Count(9);
    assert!(too_many.validate().is_err());
}

#[test]
fn event_payload_and_identity_invariants() {
    assert!(event().validate().is_ok());

    let mut empty_id = event();
    empty_id.id.clear();
    assert!(empty_id.validate().is_err());

    let mut bad_probability = event();
    bad_probability.payload = EventPayload::Presence {
        occupied: true,
        probability: 2.0,
    };
    assert!(bad_probability.validate().is_err());

    let mut empty_metric = event();
    empty_metric.payload = EventPayload::Vitals {
        metric: String::new(),
        value: 1.0,
        unit: "brpm".to_string(),
    };
    assert!(empty_metric.validate().is_err());
}

#[test]
fn session_invariants() {
    assert!(session().validate().is_ok());

    let mut no_nodes = session();
    no_nodes.nodes.clear();
    assert!(no_nodes.validate().is_err());

    let mut backwards = session();
    backwards.ended_unix_ms = Some(backwards.started_unix_ms - 1);
    assert!(backwards.validate().is_err());

    let mut bad_slug = session();
    bad_slug.id = Slug(String::new());
    assert!(bad_slug.validate().is_err());
}

#[test]
fn ledger_kind_obligations() {
    assert!(ledger_record(MetricKind::Gated).validate().is_ok());
    assert!(ledger_record(MetricKind::Statistical).validate().is_ok());
    assert!(ledger_record(MetricKind::Deterministic).validate().is_ok());

    let mut statistical_no_ci = ledger_record(MetricKind::Statistical);
    statistical_no_ci.ci = None;
    assert!(statistical_no_ci.validate().is_err());

    let mut statistical_no_samples = ledger_record(MetricKind::Statistical);
    statistical_no_samples.samples = 0;
    assert!(statistical_no_samples.validate().is_err());

    let mut gated_no_threshold = ledger_record(MetricKind::Gated);
    gated_no_threshold.threshold = None;
    assert!(gated_no_threshold.validate().is_err());

    let mut gated_no_verdict = ledger_record(MetricKind::Gated);
    gated_no_verdict.passed = None;
    assert!(gated_no_verdict.validate().is_err());

    let inverted = Interval {
        low: 1.0,
        high: 0.0,
    };
    assert!(inverted.validate().is_err());
}

#[test]
fn topology_referential_integrity() {
    assert!(topology().validate().is_ok());

    let mut duplicate_node = topology();
    duplicate_node.nodes.push(topology_node(1, "kitchen-01"));
    assert!(duplicate_node.validate().is_err());

    let mut missing_room = topology();
    missing_room.nodes[0].room = slug("nowhere");
    assert!(missing_room.validate().is_err());

    let mut unprovisioned = topology();
    unprovisioned.nodes[0].id = NodeId::UNPROVISIONED;
    assert!(unprovisioned.validate().is_err());

    let mut self_pair = topology();
    self_pair.pairs[0].rx = self_pair.pairs[0].tx;
    assert!(self_pair.validate().is_err());

    let mut mismatched_scope = topology();
    mismatched_scope.pairs[0].scope = PairScope::CrossFloor;
    assert!(mismatched_scope.validate().is_err());

    let mut duplicate_level = topology();
    duplicate_level.floors.push(Floor {
        id: slug("first-floor"),
        home: slug("home-01"),
        name: "First".to_string(),
        level: 0,
        elevation_m: 3.0,
    });
    assert!(duplicate_level.validate().is_err());
}

#[test]
fn calibration_and_baseline_invariants() {
    assert!(calibration().validate().is_ok());
    assert!(baseline().validate().is_ok());
    assert!(clock_model().validate().is_ok());

    let mut bad_validity = calibration();
    bad_validity.validity.end_mono_ns = bad_validity.validity.start_mono_ns;
    assert!(bad_validity.validate().is_err());

    let mut non_finite = calibration();
    non_finite.node_offsets_db[0].1 = f32::NAN;
    assert!(non_finite.validate().is_err());

    let mut length_mismatch = baseline();
    length_mismatch.phase_ref_rad.pop();
    assert!(length_mismatch.validate().is_err());

    let mut bad_drift = clock_model();
    bad_drift.drift_ppm = f64::INFINITY;
    assert!(bad_drift.validate().is_err());
}
