//! The fan-in adapter: multiplex several [`CsiSource`]s into one ordered stream.
//!
//! Fan-in is not needed live — one UDP socket already covers every node — but it
//! lets tests combine two recorded Sessions into one multi-node stream
//! (`capture-and-firmware.md` §3.2). Frames are ordered by
//! `(host_rx_mono_ns, node_id, sequence)`, the same total order
//! `data-contracts.md` §2 gives `SensingFrame` contributors.
//!
//! Each child is pulled one frame ahead, so a fan-in over already-ordered
//! sources yields a globally ordered merge.

use aquila_types::CsiEnvelope;

use crate::error::SourceError;
use crate::seam::{CsiSource, FrameOrEnd};

/// Multiplexes several sources into one stream ordered by host arrival.
pub struct FanInSource {
    sources: Vec<Box<dyn CsiSource>>,
    pending: Vec<Option<CsiEnvelope>>,
    done: Vec<bool>,
    stopped: bool,
}

impl FanInSource {
    /// Wrap a set of sources. Order of the input is irrelevant to output order.
    #[must_use]
    pub fn new(sources: Vec<Box<dyn CsiSource>>) -> Self {
        let len = sources.len();
        Self {
            sources,
            pending: (0..len).map(|_| None).collect(),
            done: vec![false; len],
            stopped: false,
        }
    }

    /// The number of multiplexed sources.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Whether the fan-in has no sources.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// The ordering key for a buffered envelope.
    fn key(env: &CsiEnvelope) -> (u64, aquila_types::NodeId, u64) {
        (
            env.host_rx_mono_ns,
            env.frame.node_id,
            env.frame.sequence,
        )
    }
}

impl CsiSource for FanInSource {
    fn recv(&mut self) -> Result<FrameOrEnd, SourceError> {
        if self.stopped {
            return Err(SourceError::Stopped);
        }

        // Pull one frame ahead from every source that has none buffered.
        for i in 0..self.sources.len() {
            if self.done[i] || self.pending[i].is_some() {
                continue;
            }
            match self.sources[i].recv() {
                Ok(FrameOrEnd::Frame(env)) => self.pending[i] = Some(env),
                Ok(FrameOrEnd::End) | Err(SourceError::Stopped) => self.done[i] = true,
                Err(e) => return Err(e),
            }
        }

        // Return the earliest buffered frame.
        let mut best: Option<usize> = None;
        for (i, slot) in self.pending.iter().enumerate() {
            let Some(env) = slot else { continue };
            match best {
                Some(b) => {
                    let incumbent = self.pending[b].as_ref().expect("buffered");
                    if Self::key(env) < Self::key(incumbent) {
                        best = Some(i);
                    }
                }
                None => best = Some(i),
            }
        }

        match best {
            Some(i) => Ok(FrameOrEnd::Frame(
                self.pending[i].take().expect("buffered"),
            )),
            None => Ok(FrameOrEnd::End),
        }
    }

    fn shutdown(&mut self) {
        self.stopped = true;
        for source in &mut self.sources {
            source.shutdown();
        }
    }
}
