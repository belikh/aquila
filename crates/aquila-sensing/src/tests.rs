//! Unit tests for the sensing core's internal modules.
//!
//! These reach the private modules directly, which is allowed because they are
//! in-crate; the seam-level behaviour is exercised through the public interface
//! by `aquila-testkit`'s tracer integration test.

use std::sync::Arc;

use aquila_metrics::InMemoryMetricSink;
use aquila_sources::{CsiSource, FrameOrEnd, SourceError};
use aquila_types::{
    Band, Clock, CoreTime, CsiEnvelope, CsiFrame, EventKind, NodeId, PhyFlags, RoomId, Slug,
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
fn calibrate_is_an_honest_stub() {
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
    let report = sensing.calibrate().unwrap();
    assert!(!report.calibrated);
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
    let room = |p: f64| crate::escalation::RoomSummary {
        room: Slug("r".into()),
        presence_probability: p,
        count: 0,
    };
    let one = crate::escalation::floor_presence_probability(&[room(0.5)]);
    let two = crate::escalation::floor_presence_probability(&[room(0.5), room(0.5)]);
    assert!(two >= one);
    assert!((one - 0.5).abs() < 1e-12);
    assert!((crate::escalation::home_presence_probability(&[one]) - one).abs() < 1e-12);
}
