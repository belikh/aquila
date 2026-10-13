//! The raw sensing sample and its core-side wrappers.
//!
//! A [`CsiFrame`] is one received 802.11 packet's complex channel state
//! information from a single node: a complex coefficient per subcarrier plus
//! capture metadata. A modem output, not raw I/Q. It is the pure node frame.
//!
//! [`CsiEnvelope`] is the core-side wrapper: the node frame stays pure, and the
//! core adds arrival data from the [`Clock`](crate::Clock) seam.
//! [`SensingFrame`] is the core's atomic unit of analysis: the CSI frames of
//! one node *or* one node-pair, time-aligned onto a common [`ClockGrid`] over a
//! fixed span. It is not a new kind of CSI — the same envelopes, placed on a
//! grid.

use core::f32::consts::PI;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::error::{require_finite_f32, ValidationError};
use crate::ids::NodeId;
use crate::primitives::{Band, PhyFlags};
use crate::time::TimeSpan;

/// The capture metadata for one CSI frame (fields other than the subcarrier
/// arrays).
///
/// The number of subcarriers is **derived** from `amplitude.len()` and is not a
/// separate stored field, so it cannot disagree with the arrays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CsiFrame {
    /// On-air identity of the sending node.
    pub node_id: NodeId,
    /// Boot epoch; incremented every boot and persisted in node NVS.
    pub boot_epoch: u16,
    /// Monotonic within `(node, boot_epoch, band, channel)`.
    pub sequence: u64,
    /// Node-local monotonic time in microseconds; not synchronised.
    pub device_time_us: u64,
    /// The radio band.
    pub band: Band,
    /// The channel number: 1–14 (2.4 GHz) or 32–177 (5 GHz).
    pub channel: u8,
    /// The channel width in MHz: 20 or 40.
    pub bandwidth_mhz: u8,
    /// The subcarrier index of the first element of `amplitude`/`phase`.
    pub subcarrier_offset: i16,
    /// Per-subcarrier amplitude, linear and arbitrary, in ascending index
    /// order. Every element is non-negative and finite.
    pub amplitude: Vec<f32>,
    /// Per-subcarrier phase, in radians, wrapped to `[-π, π]`. Per-frame phase
    /// is uncalibrated; cross-frame phase comparison is out of scope for v1.
    pub phase: Vec<f32>,
    /// Whole-frame received signal strength, in dBm.
    pub rssi_dbm: i8,
    /// Whole-frame noise floor, in dBm.
    pub noise_floor_dbm: i8,
    /// PHY status flags.
    pub phy: PhyFlags,
}

impl CsiFrame {
    /// The number of subcarriers, derived from the amplitude array.
    #[must_use]
    pub fn n_subcarriers(&self) -> usize {
        self.amplitude.len()
    }

    /// Validate the per-frame invariants that do not require neighbouring
    /// frames: array agreement, non-negativity, finiteness and phase wrapping.
    pub fn validate(&self) -> Result<(), ValidationError> {
        let n = self.amplitude.len();
        if n == 0 {
            return Err(ValidationError::Length {
                field: "amplitude",
                actual: 0,
                expected: 1,
            });
        }
        if self.phase.len() != n {
            return Err(ValidationError::Length {
                field: "phase",
                actual: self.phase.len(),
                expected: n,
            });
        }
        for (i, a) in self.amplitude.iter().enumerate() {
            require_finite_f32("amplitude", *a)?;
            if *a < 0.0 {
                return Err(ValidationError::Range {
                    field: "amplitude",
                    detail: format!("element {i} is negative"),
                });
            }
        }
        for (i, p) in self.phase.iter().enumerate() {
            require_finite_f32("phase", *p)?;
            // A small epsilon tolerates the rounding of a value exactly at the
            // wrap boundary, which is produced by atan2.
            if !(-PI..=PI).contains(p) {
                return Err(ValidationError::Range {
                    field: "phase",
                    detail: format!("element {i} is outside [-π, π]"),
                });
            }
        }
        Ok(())
    }
}

/// The core-side wrapper around a [`CsiFrame`].
///
/// The core adds arrival data using the `Clock` seam; the node frame itself is
/// kept verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CsiEnvelope {
    /// The node frame, verbatim.
    pub frame: CsiFrame,
    /// Arrival on the host monotonic clock, in nanoseconds.
    pub host_rx_mono_ns: u64,
    /// Wall time at arrival, in milliseconds; for audit only.
    pub host_rx_unix_ms: i64,
    /// Source address, for logging and diagnostics only.
    pub source_addr: Option<SocketAddr>,
}

impl CsiEnvelope {
    /// Validate the wrapped frame.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.frame.validate()
    }
}

/// The scope of a [`SensingFrame`]: one node, or one ordered node-pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensingScope {
    /// A single node's frames.
    Node(NodeId),
    /// A single ordered node-pair's frames.
    Pair(crate::ids::PairId),
}

impl SensingScope {
    /// Validate that any identifier carried by the scope is non-empty.
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            SensingScope::Node(_) => Ok(()),
            SensingScope::Pair(id) => id.validate(),
        }
    }
}

/// A uniform sampling grid: slot `k` is at
/// `origin_mono_ns + k * interval_ns`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockGrid {
    /// The monotonic time of slot 0, in nanoseconds.
    pub origin_mono_ns: u64,
    /// The spacing between adjacent slots, in nanoseconds. Must be non-zero.
    pub interval_ns: u64,
    /// The number of slots on the grid.
    pub slots: u32,
}

impl ClockGrid {
    /// Validate that the grid has a positive interval and at least one slot.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.interval_ns == 0 {
            return Err(ValidationError::Range {
                field: "ClockGrid.interval_ns",
                detail: "must be non-zero".to_string(),
            });
        }
        if self.slots == 0 {
            return Err(ValidationError::Invariant {
                detail: "ClockGrid.slots must be at least 1".to_string(),
            });
        }
        Ok(())
    }
}

/// The core's atomic unit of analysis.
///
/// The CSI frames of one node *or* one node-pair, time-aligned onto a common
/// [`ClockGrid`] over a fixed span. The input to feature extraction and
/// inference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensingFrame {
    /// The scope: one node or one node-pair.
    pub scope: SensingScope,
    /// The span the frame covers.
    pub span: TimeSpan,
    /// The common clock grid.
    pub grid: ClockGrid,
    /// The contributing CSI frames, ordered by
    /// `(host_rx_mono_ns, node_id, sequence)`.
    pub frames: Vec<CsiEnvelope>,
}

impl SensingFrame {
    /// Validate the scope, span, grid and contributing frames.
    ///
    /// When the scope is a single node, every contributing frame must belong to
    /// that node. Pair membership cannot be checked without a topology and is
    /// left to the loading crate.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.scope.validate()?;
        self.span.validate()?;
        self.grid.validate()?;
        if let SensingScope::Node(node) = &self.scope {
            for frame in &self.frames {
                if frame.frame.node_id != *node {
                    return Err(ValidationError::Invariant {
                        detail: "SensingFrame holds a frame from outside its node scope"
                            .to_string(),
                    });
                }
            }
        }
        for frame in &self.frames {
            frame.validate()?;
        }
        Ok(())
    }
}
