//! Tests for the node↔core seam.
//!
//! The replay tests pin the determinism contract; the live test exercises a
//! real UDP socket on loopback (no external network); the fan-in test pins the
//! host-arrival ordering; the control test pins an `AQCC` round trip.

use std::net::UdpSocket;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use aquila_session::{Manifest, SessionReader, SessionWriter};
use aquila_types::{
    Band, BuildId, Clock, ContractVersion, CoreTime, CsiFrame, NodeId, PhyFlags, Session, Slug,
};
use aquila_wire::{
    encode_control, encode_datagram, result_message, AckStatus, ControlBody, ControlMessage,
    SyncPing,
};

use crate::*;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A clock whose `now` is fixed at construction: deterministic replay stamps
/// every frame with the same instant.
#[derive(Debug)]
struct FixedClock(u64);

impl Clock for FixedClock {
    fn now(&self) -> CoreTime {
        CoreTime(self.0)
    }
}

/// A clock that advances by a fixed step on every read, for gap tests.
#[derive(Debug)]
struct SteppingClock(AtomicU64, u64);

impl SteppingClock {
    fn new(start: u64, step: u64) -> Arc<Self> {
        Arc::new(Self(AtomicU64::new(start), step))
    }
}

impl Clock for SteppingClock {
    fn now(&self) -> CoreTime {
        CoreTime(self.0.fetch_add(self.1, Ordering::Relaxed))
    }
}

/// A representative 64-subcarrier frame for `node`.
fn frame(node: u16, sequence: u64) -> CsiFrame {
    CsiFrame {
        node_id: NodeId(node),
        boot_epoch: 1,
        sequence,
        device_time_us: sequence * 1_000,
        band: Band::B2G4,
        channel: 6,
        bandwidth_mhz: 20,
        subcarrier_offset: -32,
        i_q: (0..128).map(|i| ((i * 5 + 7) % 200) as u8 as i8).collect(),
        rssi_dbm: -40,
        noise_floor_dbm: -90,
        phy: PhyFlags(PhyFlags::HT),
    }
}

/// Build a `.aqs` session carrying `frames`, one CSI record each.
fn session_bytes(frames: &[CsiFrame]) -> Vec<u8> {
    let session = Session {
        id: Slug("replay-fixture".into()),
        name: "Replay fixture".into(),
        home: Slug("demo-home".into()),
        nodes: vec![NodeId(1), NodeId(2)],
        started_unix_ms: 0,
        ended_unix_ms: None,
        contract: ContractVersion::new(1, 0),
        build: BuildId {
            git_sha: "deadbeef".into(),
            dirty: false,
        },
        config_hash: [0u8; 32],
        seed: 7,
        artifacts: vec![],
        ground_truth: None,
        notes: String::new(),
    };
    let mut writer = SessionWriter::new(&Manifest::new(session)).unwrap();
    for (i, f) in frames.iter().enumerate() {
        writer
            .write_frame(i as u64 * 1_000, &encode_datagram(f))
            .unwrap();
    }
    writer.finish().unwrap()
}

/// Drain a source into its decoded envelopes.
fn drain(source: &mut dyn CsiSource) -> Vec<aquila_types::CsiEnvelope> {
    let mut out = Vec::new();
    while let Ok(FrameOrEnd::Frame(env)) = source.recv() {
        out.push(env);
    }
    out
}

// ---------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------

#[test]
fn replay_decodes_frames_and_preserves_order() {
    let frames = [frame(1, 0), frame(1, 1), frame(2, 0)];
    let bytes = session_bytes(&frames);
    let reader = SessionReader::from_bytes(&bytes).unwrap();
    let clock = Arc::new(FixedClock(1_000_000_000));
    let mut source = ReplaySource::from_reader(reader, ReplayMode::Deterministic, clock);

    assert_eq!(source.frame_count(), 3);
    assert_eq!(source.peek_arrival_us(), Some(0));
    let envelopes = drain(&mut source);

    let decoded: Vec<CsiFrame> = envelopes.iter().map(|e| e.frame.clone()).collect();
    assert_eq!(decoded, frames, "replayed frames must match the originals");
    assert_eq!(envelopes[0].host_rx_mono_ns, 1_000_000_000);
    assert_eq!(source.recv().unwrap(), FrameOrEnd::End);
}

#[test]
fn deterministic_replay_is_stable_across_runs() {
    let frames = [frame(1, 0), frame(2, 3), frame(1, 1)];
    let bytes = session_bytes(&frames);

    let run = || {
        let reader = SessionReader::from_bytes(&bytes).unwrap();
        let clock = Arc::new(FixedClock(42));
        let mut source = ReplaySource::from_reader(reader, ReplayMode::Deterministic, clock);
        drain(&mut source)
    };

    assert_eq!(run(), run(), "deterministic replay must be byte-identical");
}

#[test]
fn replay_honours_shutdown() {
    let bytes = session_bytes(&[frame(1, 0)]);
    let reader = SessionReader::from_bytes(&bytes).unwrap();
    let mut source =
        ReplaySource::from_reader(reader, ReplayMode::Deterministic, Arc::new(FixedClock(0)));
    source.shutdown();
    let err = source.recv().unwrap_err();
    assert_eq!(err.kind(), "stopped");
}

#[test]
fn realtime_replay_uses_the_injected_clock() {
    let frames = [frame(1, 0), frame(1, 1)];
    let bytes = session_bytes(&frames);
    let reader = SessionReader::from_bytes(&bytes).unwrap();
    // A stepping clock returns immediately from the pacing loop.
    let clock = SteppingClock::new(1_000_000, 100_000);
    let mut source = ReplaySource::from_reader(reader, ReplayMode::Realtime, clock);
    let envelopes = drain(&mut source);
    assert_eq!(envelopes.len(), 2);
    assert!(
        envelopes[0].host_rx_mono_ns <= envelopes[1].host_rx_mono_ns,
        "realtime arrivals must be non-decreasing"
    );
}

// ---------------------------------------------------------------------------
// Live UDP on loopback
// ---------------------------------------------------------------------------

#[test]
fn udp_live_source_counts_bad_frames_and_keeps_going() {
    let mut source = UdpLiveSource::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(FixedClock(5)),
        LiveConfig::default(),
    )
    .unwrap();
    let addr = source.local_addr().unwrap();
    let stats = source.stats();

    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    // A malformed datagram first: the stream must survive it.
    sender.send_to(&[0u8, 1, 2, 3], addr).unwrap();
    let good = frame(9, 1);
    sender.send_to(&encode_datagram(&good), addr).unwrap();

    let env = match source.recv().unwrap() {
        FrameOrEnd::Frame(env) => env,
        FrameOrEnd::End => panic!("live source never ends"),
    };
    assert_eq!(env.frame, good);
    assert_eq!(env.host_rx_mono_ns, 5);
    let snapshot = stats.snapshot();
    assert_eq!(snapshot.decoded, 1);
    assert_eq!(snapshot.bad_frame, 1);
    assert!(snapshot.datagrams >= 2);
}

#[test]
fn udp_live_source_shutdown_returns_stopped() {
    let mut source = UdpLiveSource::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(FixedClock(0)),
        LiveConfig::default(),
    )
    .unwrap();
    source.shutdown();
    assert_eq!(source.recv().unwrap_err().kind(), "stopped");
}

// ---------------------------------------------------------------------------
// Fan-in
// ---------------------------------------------------------------------------

#[test]
fn fan_in_orders_by_host_arrival() {
    let early = session_bytes(&[frame(2, 0), frame(2, 1)]);
    let late = session_bytes(&[frame(1, 0)]);
    let early_source = ReplaySource::from_reader(
        SessionReader::from_bytes(&early).unwrap(),
        ReplayMode::Deterministic,
        Arc::new(FixedClock(1_000)),
    );
    let late_source = ReplaySource::from_reader(
        SessionReader::from_bytes(&late).unwrap(),
        ReplayMode::Deterministic,
        Arc::new(FixedClock(2_000)),
    );
    let mut fan_in = FanInSource::new(vec![Box::new(late_source), Box::new(early_source)]);
    assert_eq!(fan_in.len(), 2);

    let nodes: Vec<u16> = drain(&mut fan_in)
        .iter()
        .map(|e| e.frame.node_id.0)
        .collect();
    assert_eq!(nodes, vec![2, 2, 1], "earliest host arrival first");
}

// ---------------------------------------------------------------------------
// Control plane
// ---------------------------------------------------------------------------

#[test]
fn control_round_trips_over_loopback() {
    let core = NodeControl::bind_any().unwrap();
    let mut node = NodeControl::bind_any().unwrap();
    node.set_read_timeout(None).unwrap();

    let message = ControlMessage {
        request_id: 0xABCD,
        body: ControlBody::SyncPing(SyncPing { t1_us: 123_456 }),
    };
    core.send(node.local_addr().unwrap(), &message).unwrap();

    let (received, from) = node.recv().unwrap();
    assert_eq!(received, message);
    assert_eq!(from, core.local_addr().unwrap());
}

#[test]
fn control_request_skips_unrelated_replies_and_matches_the_id() {
    let mut core = NodeControl::bind_any().unwrap();
    let core_addr = core.local_addr().unwrap();
    let node = UdpSocket::bind("127.0.0.1:0").unwrap();
    let node_addr = node.local_addr().unwrap();

    // A stale reply with a different request id is queued first; `request` must
    // skip it and return the one that echoes the request's id.
    let unrelated = encode_control(&result_message(1, AckStatus::Active, "stale")).unwrap();
    let reply = encode_control(&result_message(42, AckStatus::Active, "ok")).unwrap();
    node.send_to(&unrelated, core_addr).unwrap();
    node.send_to(&reply, core_addr).unwrap();

    let request = ControlMessage {
        request_id: 42,
        body: ControlBody::SyncPing(SyncPing { t1_us: 7 }),
    };
    let got = core
        .request(node_addr, &request, Duration::from_millis(500))
        .unwrap();
    assert_eq!(got, result_message(42, AckStatus::Active, "ok"));
}

#[test]
fn control_request_times_out_without_a_reply() {
    let mut core = NodeControl::bind_any().unwrap();
    let node = UdpSocket::bind("127.0.0.1:0").unwrap();
    let request = ControlMessage {
        request_id: 7,
        body: ControlBody::SyncPing(SyncPing { t1_us: 0 }),
    };
    let error = core
        .request(
            node.local_addr().unwrap(),
            &request,
            Duration::from_millis(50),
        )
        .unwrap_err();
    assert_eq!(error.kind(), "timeout");
}
