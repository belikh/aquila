//! Unit tests for the sensing core's internal modules.
//!
//! These reach the private modules directly, which is allowed because they are
//! in-crate; the seam-level behaviour is exercised through the public interface
//! by `aquila-testkit`'s tracer integration test.

use std::sync::Arc;

use aquila_metrics::InMemoryMetricSink;
use aquila_sources::{CsiSource, FrameOrEnd, SourceError};
use aquila_types::{
    Band, Clock, CoreTime, CsiEnvelope, CsiFrame, Estimate, EventKind, NodeId, PhyFlags, RoomId,
    Slug,
};

use crate::clock::ManualClock;
use crate::config::SensingConfig;
use crate::fusion::{self, NodeEvidence};
use crate::sink::Sink;
use crate::time::TimeSync;
use crate::window::WindowAssembler;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn envelope(node: u16, sequence: u64, arrival_ns: u64) -> CsiEnvelope {
    CsiEnvelope {
        frame: CsiFrame {
            node_id: NodeId(node),
            boot_epoch: 1,
            sequence,
            device_time_us: sequence * 50_000,
            band: Band::B2G4,
            channel: 6,
            bandwidth_mhz: 20,
            subcarrier_offset: -32,
            amplitude: (0..64).map(|k| 1.0 + (k as f32) * 0.01).collect(),
            phase: (0..64).map(|k| (k as f32 / 64.0) - 0.5).collect(),
            rssi_dbm: -40,
            noise_floor_dbm: -90,
            phy: PhyFlags(PhyFlags::HT),
        },
        host_rx_mono_ns: arrival_ns,
        host_rx_unix_ms: 0,
        source_addr: None,
    }
}

/// A source that yields a fixed vector of envelopes.
struct VecSource {
    frames: std::vec::IntoIter<CsiEnvelope>,
    stopped: bool,
}

impl VecSource {
    fn new(frames: Vec<CsiEnvelope>) -> Self {
        Self {
            frames: frames.into_iter(),
            stopped: false,
        }
    }
}

impl CsiSource for VecSource {
    fn recv(&mut self) -> Result<FrameOrEnd, SourceError> {
        if self.stopped {
            return Err(SourceError::Stopped);
        }
        match self.frames.next() {
            Some(frame) => Ok(FrameOrEnd::Frame(frame)),
            None => Ok(FrameOrEnd::End),
        }
    }
    fn shutdown(&mut self) {
        self.stopped = true;
    }
}

/// A sink that counts events.
#[derive(Default)]
struct CountingSink {
    count: usize,
}

impl Sink for CountingSink {
    fn emit(&mut self, _event: &aquila_types::Event) -> Result<(), crate::error::SinkError> {
        self.count += 1;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Time sync
// ---------------------------------------------------------------------------

#[test]
fn time_sync_recovers_an_affine_map() {
    let mut sync = TimeSync::new(64, 2);
    // core = 2 * device + 5 (an artificial factor for a crisp assertion).
    for i in 0..16u64 {
        sync.observe(NodeId(1), i * 1_000, 2 * i * 1_000 + 5);
    }
    let fit = sync.model(NodeId(1)).expect("enough samples");
    assert!((fit.slope - 2.0).abs() < 1e-9);
    assert!((fit.intercept_ns - 5.0).abs() < 1e-6);
    assert_eq!(fit.correct_core_ns(1_000), 2_005);
    assert_eq!(fit.samples, 16);
}

#[test]
fn time_sync_publishes_no_model_below_min_samples() {
    let mut sync = TimeSync::new(64, 8);
    for i in 0..4u64 {
        sync.observe(NodeId(1), i, i);
    }
    assert!(sync.model(NodeId(1)).is_none());
    assert_eq!(sync.weight(NodeId(1)), 1.0);
}

// ---------------------------------------------------------------------------
// Fusion
// ---------------------------------------------------------------------------

fn evidence(node: u16, logit: f64, weight: f64) -> NodeEvidence {
    let p = fusion::sigmoid(logit);
    let room: RoomId = Slug("room".into());
    NodeEvidence {
        node: NodeId(node),
        room,
        presence_logit: logit,
        count_pmf: fusion::node_count_pmf(p, 8),
        weight,
        frames: 10,
    }
}

#[test]
fn presence_pooling_is_permutation_invariant() {
    let a = vec![
        evidence(1, 2.0, 1.0),
        evidence(2, -1.0, 0.5),
        evidence(3, 0.5, 0.8),
    ];
    let mut b = a.clone();
    b.reverse();
    let pa = fusion::pooled_presence_probability(0.0, &a, 4.0);
    let pb = fusion::pooled_presence_probability(0.0, &b, 4.0);
    assert!((pa - pb).abs() < 1e-12);
}

#[test]
fn zero_weight_is_inert() {
    let base = fusion::pooled_presence_probability(0.0, &[evidence(1, 3.0, 1.0)], 4.0);
    let with_inert = fusion::pooled_presence_probability(
        0.0,
        &[evidence(1, 3.0, 1.0), evidence(2, 9.0, 0.0)],
        4.0,
    );
    assert!((base - with_inert).abs() < 1e-12);
}

#[test]
fn empty_input_is_unknown_not_absent() {
    let p = fusion::pooled_presence_probability(0.0, &[], 4.0);
    assert!((p - 0.5).abs() < 1e-12, "prior 0 must be p=0.5");
}

#[test]
fn adding_positive_evidence_never_lowers_presence() {
    let one = fusion::pooled_presence_probability(0.0, &[evidence(1, 1.0, 1.0)], 4.0);
    let two = fusion::pooled_presence_probability(
        0.0,
        &[evidence(1, 1.0, 1.0), evidence(2, 2.0, 1.0)],
        4.0,
    );
    assert!(two >= one);
}

#[test]
fn count_mixture_is_a_distribution() {
    let ev = vec![evidence(1, 2.0, 1.0), evidence(2, 1.0, 1.0)];
    let pmf = fusion::mix_count_pmf(&ev, 8);
    let sum: f64 = pmf.probs.iter().sum();
    assert!((sum - 1.0).abs() < 1e-9);
    let (map, _) = pmf.map();
    assert!(map <= 8);
}

#[test]
fn fusion_weight_folds_calibration_quality_and_clock_health() {
    assert_eq!(fusion::node_fusion_weight(0.0, 1.0), 0.0);
    assert_eq!(fusion::node_fusion_weight(1.0, 0.0), 0.0);
    assert!((fusion::node_fusion_weight(0.8, 0.5) - 0.4).abs() < 1e-12);
    // Non-finite inputs are no evidence, never a NaN weight.
    assert_eq!(fusion::node_fusion_weight(f64::NAN, 1.0), 0.0);
    assert_eq!(fusion::node_fusion_weight(1.0, f64::NAN), 0.0);
}

#[test]
fn torso_fusion_with_one_node_is_unknown_not_a_pose() {
    let geometry = fusion::TorsoGeometry::adult();
    let single = [fusion::NodeTorsoRanges {
        node: NodeId(1),
        position_m: [0.0, 0.0],
        ranges_m: [1.0, 1.2, 0.9, 1.1],
        weight: 1.0,
    }];
    assert!(
        fusion::fuse_torso_ranges(&single, &geometry).is_none(),
        "a single view cannot place a torso; no pose may be fabricated"
    );
}

#[test]
fn torso_fusion_recovers_an_exact_torso() {
    let geometry = fusion::TorsoGeometry::adult();
    let centre = [2.0_f64, 1.5];
    let yaw = 0.4_f64;
    let keypoints = geometry.keypoints_m(centre, yaw);
    let node_positions = [[0.3_f64, 0.3], [0.3, 3.7]];
    let nodes: Vec<fusion::NodeTorsoRanges> = node_positions
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let ranges = keypoints.map(|kp| {
                let dx = kp[0] - node[0];
                let dy = kp[1] - node[1];
                (dx * dx + dy * dy).sqrt()
            });
            fusion::NodeTorsoRanges {
                node: NodeId(i as u16 + 1),
                position_m: *node,
                ranges_m: ranges,
                weight: 1.0,
            }
        })
        .collect();
    let estimate = fusion::fuse_torso_ranges(&nodes, &geometry).expect("a fit");
    let centre_error = ((estimate.centre_m[0] - centre[0]).powi(2)
        + (estimate.centre_m[1] - centre[1]).powi(2))
    .sqrt();
    assert!(centre_error < 0.05, "centre error {centre_error} m");
    let yaw_error = (estimate.yaw_rad - yaw).abs();
    assert!(yaw_error < 0.1, "yaw error {yaw_error} rad");
    assert!(estimate.residual_rms_m < 0.01);
}

// ---------------------------------------------------------------------------
// Window assembly
// ---------------------------------------------------------------------------
#[test]
fn assembler_closes_a_window_on_the_boundary() {
    let cfg = SensingConfig::default();
    let mut assembler = WindowAssembler::new(&cfg);
    assert!(assembler.push(envelope(1, 0, 0)).is_none());
    assert!(assembler.push(envelope(1, 1, 500_000_000)).is_none());
    let batch = assembler
        .push(envelope(1, 2, 1_000_000_000))
        .expect("crossing into the next hop closes the window");
    assert_eq!(batch.frame_count(), 2);
    assert_eq!(batch.start, CoreTime(0));
    assert_eq!(batch.frames.len(), 1);
    batch.frames[0].validate().unwrap();

    let final_batch = assembler.finish().expect("the tail window is closed");
    assert_eq!(final_batch.frame_count(), 1);
}

#[test]
fn assembler_groups_multiple_nodes() {
    let cfg = SensingConfig::default();
    let mut assembler = WindowAssembler::new(&cfg);
    assembler.push(envelope(2, 0, 0));
    assembler.push(envelope(1, 0, 10));
    let batch = assembler.push(envelope(1, 1, 1_000_000_000)).unwrap();
    // Two nodes were grouped into the closed window, in ascending node order.
    assert_eq!(batch.frames.len(), 2);
    assert_eq!(
        batch.frames[0].scope,
        aquila_types::SensingScope::Node(NodeId(1))
    );
    assert_eq!(
        batch.frames[1].scope,
        aquila_types::SensingScope::Node(NodeId(2))
    );
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// A minimal two-node, one-room contract topology.
fn one_room_topology() -> aquila_types::Topology {
    use aquila_types::{Floor, Home, MacAddr, Node, Room, RoomKind, CONTRACT_VERSION};
    let home = Slug("home".into());
    let floor = Slug("ground".into());
    let room = Slug("lounge".into());
    let firmware = aquila_types::FirmwareRef {
        version: "test".into(),
        sha256: [0u8; 32],
    };
    let node = |id: u16| Node {
        id: NodeId(id),
        room: room.clone(),
        name: format!("node-{id}"),
        mac: MacAddr([0, 0, 0, 0, 0, id as u8]),
        device: aquila_types::DeviceType::Esp32S3,
        firmware: firmware.clone(),
        pos_room_m: Some([0.0, 0.0, 2.4]),
        installed_unix_ms: 0,
    };
    aquila_types::Topology {
        home: Home {
            id: home.clone(),
            name: "Home".into(),
            time_zone: "Australia/Sydney".into(),
            contract: CONTRACT_VERSION,
        },
        floors: vec![Floor {
            id: floor.clone(),
            home: home.clone(),
            name: "Ground".into(),
            level: 0,
            elevation_m: 0.0,
        }],
        rooms: vec![Room {
            id: room.clone(),
            floor,
            name: "Lounge".into(),
            kind: RoomKind::Living,
            origin_home_m: [0.0, 0.0, 0.0],
            yaw_deg: 0.0,
            area_m2: Some(20.0),
        }],
        nodes: vec![node(1), node(2)],
        pairs: vec![],
    }
}

#[test]
fn engine_steps_and_emits_measurements() {
    let frames: Vec<CsiEnvelope> = (0..3u64)
        .flat_map(|seq| {
            [
                envelope(1, seq, seq * 50_000_000),
                envelope(2, seq, seq * 50_000_000 + 10),
            ]
        })
        .collect();
    let source = Box::new(VecSource::new(frames));
    let clock = Arc::new(ManualClock::new(0));
    let sink = Box::new(CountingSink::default());
    let metrics = Box::new(InMemoryMetricSink::new());

    let mut sensing = crate::engine::Sensing::new(
        source,
        clock,
        sink,
        metrics,
        None,
        None,
        one_room_topology(),
        SensingConfig::default(),
    )
    .expect("valid core");

    let mut windows = 0;
    loop {
        let report = sensing.step().unwrap();
        windows += report.windows_emitted;
        if report.ended {
            break;
        }
    }
    assert_eq!(windows, 1, "one window was closed at end of stream");
    assert_eq!(sensing.windows_processed(), 1);
}

#[test]
fn calibrate_refuses_empty_inputs_honestly() {
    let source = Box::new(VecSource::new(vec![]));
    let clock = Arc::new(ManualClock::new(0));
    let sink = Box::new(CountingSink::default());
    let metrics = Box::new(InMemoryMetricSink::new());
    let mut sensing = crate::engine::Sensing::new(
        source,
        clock,
        sink,
        metrics,
        None,
        None,
        one_room_topology(),
        SensingConfig::default(),
    )
    .unwrap();
    let request = crate::calibration::CalibrationRequest::new(
        "0".repeat(40),
        "cal-test",
        "2026-10-14T00:00:00Z",
    );
    let report = sensing
        .calibrate(&request, &crate::calibration::CalibrationInputs::default())
        .unwrap();
    assert!(!report.calibrated);
    assert!(report.artifact.is_none());
}

#[test]
fn manual_clock_advances() {
    let clock = ManualClock::new(10);
    assert_eq!(clock.now(), CoreTime(10));
    clock.advance(5);
    assert_eq!(clock.now(), CoreTime(15));
}

#[test]
fn event_kind_is_used_by_the_engine_module() {
    // Guards the public vocabulary against accidental renames.
    let _ = EventKind::PresenceChanged;
}

#[test]
fn clamp_logit_is_bounded_and_finite() {
    assert_eq!(crate::fusion::clamp_logit(10.0, 4.0), 4.0);
    assert_eq!(crate::fusion::clamp_logit(-10.0, 4.0), -4.0);
    assert_eq!(crate::fusion::clamp_logit(f64::NAN, 4.0), 0.0);
}

#[test]
fn escalation_soft_or_is_monotone() {
    let room = |p: f64| {
        crate::escalation::RoomSummary::covered(Slug("r".into()), p, fusion::node_count_pmf(p, 8))
    };
    let one = crate::escalation::floor_presence_probability(&[room(0.5)]);
    let two = crate::escalation::floor_presence_probability(&[room(0.5), room(0.5)]);
    assert!(two >= one);
    assert!((one - 0.5).abs() < 1e-12);
    assert!((crate::escalation::home_presence_probability(&[one]) - one).abs() < 1e-12);
}

#[test]
fn floor_count_is_the_convolution_of_room_pmfs() {
    let room = |id: &str, p: f64| {
        crate::escalation::RoomSummary::covered(Slug(id.into()), p, fusion::node_count_pmf(p, 8))
    };
    let summary = crate::escalation::aggregate_floor(
        Slug("ground".into()),
        &[room("kitchen", 0.999), room("lounge", 0.999)],
        0.15,
        0.8,
        8,
    );
    // Two rooms each almost certainly holding one person convolve to two.
    assert_eq!(summary.count, Some(2));
    assert!(summary.presence_probability.unwrap_or(0.0) > 0.99);
    // A single occupied room yields its own MAP count.
    let single = crate::escalation::aggregate_floor(
        Slug("ground".into()),
        &[room("kitchen", 0.999), room("lounge", 0.01)],
        0.15,
        0.8,
        8,
    );
    assert_eq!(single.count, Some(1));
}

#[test]
fn ambiguous_room_attribution_raises_a_conflict() {
    let room = |id: &str, p: f64| {
        crate::escalation::RoomSummary::covered(Slug(id.into()), p, fusion::node_count_pmf(p, 8))
    };
    let ambiguous = crate::escalation::aggregate_floor(
        Slug("ground".into()),
        &[room("kitchen", 0.9), room("lounge", 0.9)],
        0.15,
        0.8,
        8,
    );
    assert!(
        ambiguous.attribution.is_none(),
        "a near-tie must not name a false confident room"
    );
    assert!(matches!(
        ambiguous.conflicts.as_slice(),
        [crate::escalation::Conflict::AmbiguousRooms { .. }]
    ));
    // The floor's aggregate presence and count are still reported: the
    // ambiguity is about which room, not whether the floor is occupied.
    assert!(ambiguous.presence_probability.unwrap_or(0.0) > 0.9);
    assert_eq!(ambiguous.count, Some(2));

    // A clear winner is attributed, with no conflict.
    let clear = crate::escalation::aggregate_floor(
        Slug("ground".into()),
        &[room("kitchen", 0.99), room("lounge", 0.1)],
        0.15,
        0.8,
        8,
    );
    assert_eq!(clear.attribution, Some(Slug("kitchen".into())));
    assert!(clear.conflicts.is_empty());
}

#[test]
fn uncovered_room_forces_unknown_floor_count() {
    let covered =
        crate::escalation::RoomSummary::covered(Slug("kitchen".into()), 0.99, fusion::node_count_pmf(0.99, 8));
    let uncovered = crate::escalation::RoomSummary::uncovered(Slug("lounge".into()), 8);
    let summary = crate::escalation::aggregate_floor(
        Slug("ground".into()),
        &[covered, uncovered],
        0.15,
        0.8,
        8,
    );
    assert_eq!(summary.count, None, "an uncovered room must not be counted as zero");
    // Coverage is not conflation: presence is still the soft-OR over the
    // covered room only.
    assert!(summary.presence_probability.unwrap_or(0.0) > 0.9);
}

#[test]
fn no_covered_rooms_is_unknown_not_absent() {
    let summary = crate::escalation::aggregate_floor(
        Slug("ground".into()),
        &[crate::escalation::RoomSummary::uncovered(Slug("lounge".into()), 8)],
        0.15,
        0.8,
        8,
    );
    assert!(summary.presence_probability.is_none());
    assert!(summary.count.is_none());
}

#[test]
fn floor_vitals_take_the_highest_confidence() {
    let estimate = |value: f32, confidence: f32| Estimate { value, confidence };
    let rooms = vec![
        crate::escalation::RoomVitals {
            room: Slug("kitchen".into()),
            breathing: Some(estimate(15.0, 0.4)),
            heart_rate: None,
        },
        crate::escalation::RoomVitals {
            room: Slug("lounge".into()),
            breathing: Some(estimate(16.0, 0.9)),
            heart_rate: Some(estimate(70.0, 0.5)),
        },
    ];
    let vitals = crate::escalation::aggregate_floor_vitals(&rooms, 0.0);
    assert_eq!(vitals.breathing.as_ref().unwrap().1.value, 16.0);
    assert_eq!(vitals.heart_rate.as_ref().unwrap().1.value, 70.0);
    // A confidence floor excludes a low-confidence estimate.
    let filtered = crate::escalation::aggregate_floor_vitals(&rooms, 0.95);
    assert!(filtered.breathing.is_none());
    assert!(filtered.heart_rate.is_none());
}
