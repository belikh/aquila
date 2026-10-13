//! The live adapter: one UDP socket on the CSI data port, all nodes.
//!
//! The node streams one CSI frame per datagram to the core's data port
//! (`capture-and-firmware.md` §5). One socket serves every node; frames are
//! demultiplexed by the `node_id` carried in the datagram. The adapter tolerates
//! and counts malformed or unknown-version datagrams rather than ending the
//! stream, and never blocks a sender: the socket is non-blocking via a read
//! timeout and a pending pull simply loops until a frame or a stop arrives.
//!
//! Time comes only from the injected [`Clock`] (`capture-and-firmware.md` §3.4);
//! the wall clock is never read here.

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use aquila_types::{Clock, CsiEnvelope};
use aquila_wire::{decode_datagram, WireError, MAX_DATAGRAM_LEN};

use crate::error::SourceError;
use crate::seam::{CsiSource, FrameOrEnd};

/// Tuning for a [`UdpLiveSource`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveConfig {
    /// How long a single `recv_from` waits before the pull loops and re-checks
    /// the stop flag. Also the granularity at which `shutdown` unblocks a
    /// pending pull.
    pub read_timeout: Duration,
    /// The receive buffer size, in bytes. Datagrams larger than this are
    /// truncated by the kernel and fail the length checks (counted as bad
    /// frames), never panicking.
    pub buffer_bytes: usize,
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            read_timeout: Duration::from_millis(50),
            buffer_bytes: MAX_DATAGRAM_LEN,
        }
    }
}

/// Monotonic counters describing a [`UdpLiveSource`]'s activity.
///
/// Every field is an atomic so a monitoring thread can read the stats while the
/// pulling thread runs, without a lock.
#[derive(Debug, Default)]
pub struct LiveStats {
    datagrams: AtomicU64,
    decoded: AtomicU64,
    dropped: AtomicU64,
    bad_version: AtomicU64,
    bad_frame: AtomicU64,
}

impl LiveStats {
    /// Total datagrams received from the socket.
    #[must_use]
    pub fn datagrams(&self) -> u64 {
        self.datagrams.load(Ordering::Relaxed)
    }

    /// Datagrams that decoded to a CSI frame.
    #[must_use]
    pub fn decoded(&self) -> u64 {
        self.decoded.load(Ordering::Relaxed)
    }

    /// Frames dropped before ingest. The live adapter itself does not drop; this
    /// counter is reserved for sequence-gap accounting by the core
    /// (`capture-and-firmware.md` §5.4).
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Datagrams rejected for an unsupported wire major version.
    #[must_use]
    pub fn bad_version(&self) -> u64 {
        self.bad_version.load(Ordering::Relaxed)
    }

    /// Datagrams that were otherwise malformed.
    #[must_use]
    pub fn bad_frame(&self) -> u64 {
        self.bad_frame.load(Ordering::Relaxed)
    }

    /// A consistent-at-a-glance copy of every counter.
    #[must_use]
    pub fn snapshot(&self) -> LiveStatsSnapshot {
        LiveStatsSnapshot {
            datagrams: self.datagrams(),
            decoded: self.decoded(),
            dropped: self.dropped(),
            bad_version: self.bad_version(),
            bad_frame: self.bad_frame(),
        }
    }

    /// Record `count` dropped frames (for gap accounting by a caller).
    pub fn record_dropped(&self, count: u64) {
        self.dropped.fetch_add(count, Ordering::Relaxed);
    }
}

/// A plain-value copy of [`LiveStats`], for assertions and logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveStatsSnapshot {
    /// Total datagrams received.
    pub datagrams: u64,
    /// Datagrams decoded to a frame.
    pub decoded: u64,
    /// Frames recorded as dropped by a caller.
    pub dropped: u64,
    /// Datagrams rejected for an unsupported version.
    pub bad_version: u64,
    /// Datagrams that were otherwise malformed.
    pub bad_frame: u64,
}

/// Serves all nodes from one UDP socket on the CSI data port.
pub struct UdpLiveSource {
    socket: UdpSocket,
    clock: Arc<dyn Clock>,
    stats: Arc<LiveStats>,
    stopped: bool,
    buffer: Vec<u8>,
}

impl UdpLiveSource {
    /// Bind the CSI data port and prepare to serve all nodes from one socket.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Io`] if the address cannot be bound.
    pub fn bind(
        addr: SocketAddr,
        clock: Arc<dyn Clock>,
        cfg: LiveConfig,
    ) -> Result<Self, SourceError> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_read_timeout(Some(cfg.read_timeout))?;
        Ok(Self {
            socket,
            clock,
            stats: Arc::new(LiveStats::default()),
            stopped: false,
            buffer: vec![0u8; cfg.buffer_bytes],
        })
    }

    /// The actual bound address; useful when binding to port `0` in tests.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Io`] if the socket has no local address.
    pub fn local_addr(&self) -> Result<SocketAddr, SourceError> {
        Ok(self.socket.local_addr()?)
    }

    /// The shared activity counters.
    #[must_use]
    pub fn stats(&self) -> Arc<LiveStats> {
        Arc::clone(&self.stats)
    }
}

impl CsiSource for UdpLiveSource {
    fn recv(&mut self) -> Result<FrameOrEnd, SourceError> {
        loop {
            if self.stopped {
                return Err(SourceError::Stopped);
            }
            match self.socket.recv_from(&mut self.buffer) {
                Ok((len, from)) => {
                    self.stats.datagrams.fetch_add(1, Ordering::Relaxed);
                    match decode_datagram(&self.buffer[..len]) {
                        Ok(frame) => {
                            self.stats.decoded.fetch_add(1, Ordering::Relaxed);
                            return Ok(FrameOrEnd::Frame(CsiEnvelope {
                                frame,
                                host_rx_mono_ns: self.clock.now().as_nanos(),
                                host_rx_unix_ms: 0,
                                source_addr: Some(from),
                            }));
                        }
                        Err(WireError::UnsupportedVersion { .. }) => {
                            self.stats.bad_version.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {
                            self.stats.bad_frame.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // Read timeout elapsed: loop to re-check the stop flag.
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(SourceError::Io(e)),
            }
        }
    }

    fn shutdown(&mut self) {
        self.stopped = true;
    }
}
