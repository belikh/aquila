//! The recorded-replay adapter: read a Session and replay it as a [`CsiSource`].
//!
//! A Session stores the exact on-wire `AQCI` datagram bytes
//! (`capture-and-firmware.md` §8.2), so replay runs the **real** `aquila-wire`
//! decoder: the wire codec is exercised by every replay test and cannot drift
//! from live.
//!
//! Two modes (`capture-and-firmware.md` §3.2):
//!
//! - [`ReplayMode::Deterministic`] ignores the recorded arrival times, re-stamps
//!   arrivals from the injected [`Clock`] and preserves record order. Identical
//!   input and clock produce byte-identical frames, which is what makes a
//!   replay metric deterministic.
//! - [`ReplayMode::Realtime`] honours recorded inter-record arrival gaps against
//!   the injected clock. It never reads the wall clock.
//!
//! No adapter reads the wall clock directly (`capture-and-firmware.md` §3.4).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use aquila_session::{Manifest, SessionReader};
use aquila_types::{Clock, CsiEnvelope};

use crate::error::SourceError;
use crate::seam::{CsiSource, FrameOrEnd};

/// How a [`ReplaySource`] paces and stamps the frames it replays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayMode {
    /// Ignore recorded arrival times, re-stamp from the injected [`Clock`] and
    /// preserve record order. Byte-identical output for identical input.
    Deterministic,
    /// Honour recorded inter-record arrival gaps in real time, against the
    /// injected clock.
    Realtime,
}

/// Replays one recorded Session through the [`CsiSource`] seam.
///
/// The session is parsed eagerly at construction; each pull decodes the next
/// stored `AQCI` datagram with [`aquila_wire::decode_datagram`].
pub struct ReplaySource {
    manifest: Manifest,
    frames: Vec<(u64, Vec<u8>)>,
    index: usize,
    mode: ReplayMode,
    clock: Arc<dyn Clock>,
    base_ns: Option<u64>,
    stopped: bool,
}

impl ReplaySource {
    /// Open a `.aqs` file and prepare to replay it.
    ///
    /// # Errors
    ///
    /// Returns a typed [`SourceError`] if the session cannot be read or parsed.
    pub fn open(path: &Path, mode: ReplayMode, clock: Arc<dyn Clock>) -> Result<Self, SourceError> {
        let reader = SessionReader::open(path)?;
        Ok(Self::from_reader(reader, mode, clock))
    }

    /// Prepare to replay an already-parsed session.
    ///
    /// The reader's CSI records are extracted in stream order; other record
    /// kinds (ground truth, events, annotations) are not replayed by the CSI
    /// seam.
    #[must_use]
    pub fn from_reader(reader: SessionReader, mode: ReplayMode, clock: Arc<dyn Clock>) -> Self {
        let manifest = reader.manifest().clone();
        let frames = reader
            .frames()
            .map(|(us, datagram)| (us, datagram.to_vec()))
            .collect();
        Self {
            manifest,
            frames,
            index: 0,
            mode,
            clock,
            base_ns: None,
            stopped: false,
        }
    }

    /// The manifest of the session being replayed.
    #[must_use]
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The replay mode.
    #[must_use]
    pub const fn mode(&self) -> ReplayMode {
        self.mode
    }

    /// The number of CSI frames in the session.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// The next frame's recorded host arrival, in microseconds, without
    /// consuming it.
    #[must_use]
    pub fn peek_arrival_us(&self) -> Option<u64> {
        self.frames.get(self.index).map(|(us, _)| *us)
    }

    /// The host monotonic timestamp, in nanoseconds, to stamp on this frame.
    fn stamp(&mut self, recorded_us: u64) -> u64 {
        match self.mode {
            ReplayMode::Deterministic => self.clock.now().as_nanos(),
            ReplayMode::Realtime => {
                let base = *self
                    .base_ns
                    .get_or_insert_with(|| self.clock.now().as_nanos());
                let target = base.saturating_add(recorded_us.saturating_mul(1_000));
                while self.clock.now().as_nanos() < target {
                    std::thread::sleep(Duration::from_millis(1));
                }
                target
            }
        }
    }
}

impl CsiSource for ReplaySource {
    fn recv(&mut self) -> Result<FrameOrEnd, SourceError> {
        if self.stopped {
            return Err(SourceError::Stopped);
        }
        let Some((recorded_us, datagram)) = self.frames.get(self.index).cloned() else {
            return Ok(FrameOrEnd::End);
        };
        let frame = aquila_wire::decode_datagram(&datagram)?;
        let host_rx_mono_ns = self.stamp(recorded_us);
        self.index += 1;
        Ok(FrameOrEnd::Frame(CsiEnvelope {
            frame,
            host_rx_mono_ns,
            host_rx_unix_ms: 0,
            source_addr: None,
        }))
    }

    fn shutdown(&mut self) {
        self.stopped = true;
    }
}
