//! Sliding-window assembly onto a core-time grid.
//!
//! A [`SensingFrame`](aquila_types::SensingFrame) is the core's atomic unit of
//! analysis: the CSI frames of one node, time-aligned onto a common clock grid
//! over a fixed span (`data-contracts.md` §2.1). This module groups arriving
//! envelopes into fixed-hop windows on the core clock and emits one
//! `SensingFrame` per contributing node when a window closes.
//!
//! Window placement is deliberately simple for slice 1: a frame belongs to the
//! window whose half-open span contains its core arrival. The window hop and the
//! grid geometry come from [`SensingConfig`](crate::SensingConfig), never from a
//! literal.

use std::collections::BTreeMap;

use aquila_types::{
    ClockGrid, CoreTime, CsiEnvelope, NodeId, SensingFrame, SensingScope, TimeSpan,
};

use crate::config::SensingConfig;

/// One closed window: its core-time span and the per-node sensing frames it
/// produced, ordered by node id.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowBatch {
    /// The inclusive start of the window on the core clock.
    pub start: CoreTime,
    /// The exclusive end of the window on the core clock.
    pub end: CoreTime,
    /// One sensing frame per contributing node, in ascending node-id order.
    pub frames: Vec<SensingFrame>,
}

impl WindowBatch {
    /// The total number of CSI envelopes across every node in the window.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.frames.iter().map(|f| f.frames.len()).sum()
    }
}

/// The currently open window.
#[derive(Debug)]
struct OpenWindow {
    slot: u64,
    start_ns: u64,
    buffers: BTreeMap<NodeId, Vec<CsiEnvelope>>,
}

/// Assembles arriving envelopes into fixed-hop [`WindowBatch`]es.
#[derive(Debug)]
pub struct WindowAssembler {
    hop_ns: u64,
    grid_interval_ns: u64,
    grid_slots: u32,
    current: Option<OpenWindow>,
}

impl WindowAssembler {
    /// Construct an assembler from the core configuration.
    #[must_use]
    pub fn new(config: &SensingConfig) -> Self {
        Self {
            hop_ns: config.window_hop_ns,
            grid_interval_ns: config.grid_interval_ns,
            grid_slots: config.grid_slots,
            current: None,
        }
    }

    /// Offer one envelope to the assembler.
    ///
    /// Returns `Some(batch)` when this arrival crossed into a new window and
    /// closed the previous one, otherwise `None`.
    pub fn push(&mut self, envelope: CsiEnvelope) -> Option<WindowBatch> {
        let arrival = envelope.host_rx_mono_ns;
        let slot = arrival / self.hop_ns;

        let mut closed = None;
        match &mut self.current {
            Some(open) if slot == open.slot => {
                open.buffers
                    .entry(envelope.frame.node_id)
                    .or_default()
                    .push(envelope);
            }
            Some(open) => {
                let start_ns = open.start_ns;
                let buffers = std::mem::take(&mut open.buffers);
                closed = Some(self.finish_buffers(start_ns, buffers));
                self.current = Some(Self::open(slot, self.hop_ns, envelope));
            }
            None => {
                self.current = Some(Self::open(slot, self.hop_ns, envelope));
            }
        }
        closed
    }

    /// Close the current window, if any, at the end of the stream.
    pub fn finish(&mut self) -> Option<WindowBatch> {
        let open = self.current.take()?;
        if open.buffers.is_empty() {
            return None;
        }
        Some(self.finish_buffers(open.start_ns, open.buffers))
    }

    /// Open a fresh window at `slot`.
    fn open(slot: u64, hop_ns: u64, first: CsiEnvelope) -> OpenWindow {
        let mut buffers = BTreeMap::new();
        buffers.insert(first.frame.node_id, vec![first]);
        OpenWindow {
            slot,
            start_ns: slot.saturating_mul(hop_ns),
            buffers,
        }
    }

    /// Turn a window's per-node buffers into sensing frames.
    fn finish_buffers(
        &self,
        start_ns: u64,
        buffers: BTreeMap<NodeId, Vec<CsiEnvelope>>,
    ) -> WindowBatch {
        let end_ns = start_ns.saturating_add(self.hop_ns);
        let span = TimeSpan {
            start_mono_ns: start_ns,
            end_mono_ns: end_ns,
            start_unix_ms: (start_ns / 1_000_000) as i64,
            end_unix_ms: (end_ns / 1_000_000) as i64,
        };
        let grid = ClockGrid {
            origin_mono_ns: start_ns,
            interval_ns: self.grid_interval_ns,
            slots: self.grid_slots,
        };

        let mut frames = Vec::with_capacity(buffers.len());
        for (node, mut envelopes) in buffers {
            envelopes
                .sort_by_key(|env| (env.host_rx_mono_ns, env.frame.node_id.0, env.frame.sequence));
            frames.push(SensingFrame {
                scope: SensingScope::Node(node),
                span,
                grid,
                frames: envelopes,
            });
        }

        WindowBatch {
            start: CoreTime(start_ns),
            end: CoreTime(end_ns),
            frames,
        }
    }
}
