//! The `record` subcommand: bind the CSI data port and write a `.aqs` Session.
//!
//! One CSI frame per datagram arrives on the data plane
//! (`capture-and-firmware.md` §5). The live source decodes each frame **and
//! retains the raw datagram**; `record` stores those exact received `AQCI`
//! bytes in the container (`capture-and-firmware.md` §8.2; `record.rs`'s
//! `last_datagram`). Replay therefore runs the real decoder on the bytes the
//! node actually sent, so a field a forward-compatible decoder ignored is still
//! preserved (`sister-issues-core.md` S17).
//!
//! Time comes only from a [`Clock`](aquila_types::Clock); the capture tool
//! stamps each record with the host monotonic arrival in microseconds.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use aquila_session::{Manifest, SessionWriter};
use aquila_sources::{CsiSource, FrameOrEnd, LiveConfig, UdpLiveSource};
use aquila_types::{BuildId, Clock, CoreTime, NodeId, Session, Slug, CONTRACT_VERSION};
use aquila_wire::CSI_DATA_PORT;

use crate::cli::Options;
use crate::error::CaptureError;

/// A monotonic host clock for the capture tool.
///
/// This is deliberately **not** the core's `Clock`: the capture tool is a
/// dev-only process and stamping arrivals from a local monotonic origin is all
/// it needs. The core's own `SystemClock` lives in `aquila-sensing`
/// (`ADR-0003` §5).
struct SystemClock {
    start: Instant,
}

impl SystemClock {
    /// Start a monotonic clock at the current instant.
    fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn now(&self) -> CoreTime {
        CoreTime(self.start.elapsed().as_nanos() as u64)
    }
}

/// Run `record`.
///
/// # Errors
///
/// Returns a typed [`CaptureError`] for an invalid address, a socket failure, a
/// session failure, or when no frame was recorded.
pub(crate) fn run(options: &Options) -> Result<(), CaptureError> {
    let out = options
        .get("out")
        .ok_or_else(|| CaptureError::Usage("record needs --out".to_string()))?;
    let bind = options.get_or("bind", "0.0.0.0");
    let port = options.get_u16_or("port", CSI_DATA_PORT)?;
    let addr: SocketAddr = format!("{bind}:{port}")
        .parse()
        .map_err(|_| CaptureError::Usage(format!("invalid bind address {bind}:{port}")))?;
    let frame_limit: u64 = match options.get("frames") {
        Some(value) => value.parse().map_err(|_| {
            CaptureError::Usage(format!("--frames must be an integer, got {value:?}"))
        })?,
        None => 1000,
    };
    let filter = parse_node_filter(options.get("nodes"))?;

    let clock: Arc<dyn Clock> = Arc::new(SystemClock::new());
    let mut source = UdpLiveSource::bind(addr, clock, LiveConfig::default())?;
    let stats = source.stats();

    // Read the frames first: the Session's node inventory is discovered from
    // what actually arrived (or taken from the filter), and the manifest must
    // name it before the container is written.
    let mut records: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut seen: BTreeSet<NodeId> = BTreeSet::new();
    while (records.len() as u64) < frame_limit {
        match source.recv()? {
            FrameOrEnd::End => break,
            FrameOrEnd::Frame(envelope) => {
                let node = envelope.frame.node_id;
                if let Some(filter) = &filter {
                    if !filter.contains(&node) {
                        continue;
                    }
                }
                seen.insert(node);
                // Store the exact received datagram, not a re-encoding: the
                // live source retains the bytes that produced `envelope.frame`.
                records.push((
                    envelope.host_rx_mono_ns / 1_000,
                    source.last_datagram().to_vec(),
                ));
            }
        }
    }

    let nodes: Vec<NodeId> = match &filter {
        Some(filter) => filter.iter().copied().collect(),
        None => seen.iter().copied().collect(),
    };
    if nodes.is_empty() {
        return Err(CaptureError::Other(
            "no frames were recorded; a Session needs at least one node".to_string(),
        ));
    }

    let now_unix_ms = unix_now_ms();
    let session_slug = options.get_or("session", "live-record");
    let home_slug = options.get_or("home", "home");
    let session = Session {
        id: Slug(session_slug.to_string()),
        name: format!("live capture {session_slug}"),
        home: Slug(home_slug.to_string()),
        nodes,
        started_unix_ms: now_unix_ms,
        ended_unix_ms: Some(now_unix_ms),
        contract: CONTRACT_VERSION,
        build: BuildId {
            git_sha: concat!("aquila-capture/", env!("CARGO_PKG_VERSION")).to_string(),
            dirty: false,
        },
        config_hash: None,
        seed: 0,
        artifacts: vec![],
        ground_truth: None,
        notes: "Recorded from live UDP by aquila-capture".to_string(),
    };
    let manifest = Manifest::new(session).with_capture_tool("aquila-capture");

    let mut writer = SessionWriter::new(&manifest)?;
    for (host_us, datagram) in &records {
        writer.write_frame(*host_us, datagram)?;
    }
    let bytes = writer.finish()?;
    std::fs::write(out, &bytes)?;

    let snapshot = stats.snapshot();
    println!(
        "recorded {} CSI frames from {} node(s) to {out}; datagrams {} decoded {} bad_frame {} bad_version {}",
        records.len(),
        manifest.session.nodes.len(),
        snapshot.datagrams,
        snapshot.decoded,
        snapshot.bad_frame,
        snapshot.bad_version,
    );
    Ok(())
}

/// Parse the optional comma-separated `--nodes` filter.
fn parse_node_filter(value: Option<&str>) -> Result<Option<BTreeSet<NodeId>>, CaptureError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let mut set = BTreeSet::new();
    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let id: u16 = part
            .parse()
            .map_err(|_| CaptureError::Usage(format!("--nodes entry {part:?} is not a node id")))?;
        set.insert(NodeId(id));
    }
    if set.is_empty() {
        return Err(CaptureError::Usage("--nodes was empty".to_string()));
    }
    Ok(Some(set))
}

/// The current Unix wall time in milliseconds, saturating at zero.
fn unix_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
