//! The raw sensing sample and its core-side wrappers.
//!
//! A [`CsiFrame`] is one received 802.11 packet's complex channel state
//! information from a single node: a raw signed int8 I/Q sample per subcarrier
//! plus capture metadata. It carries the esp-csi sample verbatim, so the node
//! performs no floating-point conversion (ADR-0005); amplitude and phase are
//! **derived** in the core. It is the pure node frame.
//!
//! [`CsiEnvelope`] is the core-side wrapper: the node frame stays pure, and the
//! core adds arrival data from the [`Clock`](crate::Clock) seam.
//! [`SensingFrame`] is the core's atomic unit of analysis: the CSI frames of
//! one node *or* one node-pair, time-aligned onto a common [`ClockGrid`] over a
//! fixed span. It is not a new kind of CSI — the same envelopes, placed on a
//! grid.

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::error::ValidationError;
use crate::ids::NodeId;
use crate::primitives::{Band, PhyFlags};
use crate::time::TimeSpan;

/// The nominal number of raw int8 I/Q counts one unit of the core's working
/// amplitude represents.
///
/// The wire and the node carry the esp-csi sample verbatim as signed int8
/// counts (ADR-0005). The esp-csi gain is arbitrary, so the core's derived
/// accessors (and `aquila-signal::decode_csi`) divide by this nominal full-scale
/// to recover the arbitrary linear unit the signal pipeline is engineered in.
/// A realistic capture fills a useful fraction of the int8 range, so the scale
/// is a count-per-unit constant, not a data-dependent gain.
pub const IQ_COUNTS_PER_UNIT: f32 = 64.0;

/// The nominal centre frequency of an 802.11 channel, in MHz.
///
/// This is the one channel→frequency map (`sister-issues-core.md` S12). The
/// 2.4 GHz band is `2407 + 5·channel` for channels 1–13, but channel 14 is
/// 2484 MHz, **not** the linear `2407 + 5·14 = 2477`. The 5 GHz band is
/// `5000 + 5·channel` for the standard UNII channels. `Band::Unknown` and
/// channel 0 map to 0 (unknown), never a fabricated frequency.
#[must_use]
pub const fn channel_centre_freq_mhz(band: Band, channel: u8) -> u32 {
    match band {
        Band::B2G4 => {
            if channel == 14 {
                2484
            } else {
                2407 + channel as u32 * 5
            }
        }
        Band::B5G => {
            if channel == 0 {
                0
            } else {
                5000 + channel as u32 * 5
            }
        }
        Band::Unknown => 0,
    }
}

/// The capture metadata for one CSI frame (fields other than the raw sample).
///
/// The number of subcarriers is **derived** from `i_q.len() / 2` and is not a
/// separate stored field, so it cannot disagree with the sample.
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
    /// The subcarrier index of the first interleaved sample in `i_q`.
    pub subcarrier_offset: i16,
    /// The raw complex channel sample per subcarrier, interleaved `Q, I`
    /// (**imaginary first**) in ascending subcarrier index order:
    /// `[Q0, I0, Q1, I1, …]`, matching the esp-csi sample. Each element is the
    /// signed int8 the capture produced; the node does not convert it.
    /// `n_subcarriers == i_q.len() / 2`. Amplitude and phase are derived from
    /// this sample ([`CsiFrame::amplitude`], [`CsiFrame::phase`]).
    pub i_q: Vec<i8>,
    /// Whole-frame received signal strength, in dBm.
    pub rssi_dbm: i8,
    /// Whole-frame noise floor, in dBm.
    pub noise_floor_dbm: i8,
    /// PHY status flags.
    pub phy: PhyFlags,
    /// ESP32 quirk: the leading `I/Q` pair is unreliable. The node marks the
    /// frame and nulls the first complex sample (two bytes); the core repeats
    /// the nulling and records the flag when decoding ([`crate::CsiFrame`] is
    /// the pure node frame, so the flag travels on the wire header's flags
    /// byte, bit 0).
    pub first_word_invalid: bool,
}

impl CsiFrame {
    /// The number of subcarriers, derived from the interleaved sample.
    #[must_use]
    pub fn n_subcarriers(&self) -> usize {
        self.i_q.len() / 2
    }

    /// The derived in-phase component of subcarrier `k`, in the core's working
    /// unit (raw counts divided by [`IQ_COUNTS_PER_UNIT`]).
    ///
    /// The sample is imaginary-first, so the in-phase byte of subcarrier `k` is
    /// the *second* byte of its pair (`i_q[2k + 1]`).
    ///
    /// Returns `0.0` when `k` is out of range.
    #[must_use]
    pub fn i_at(&self, k: usize) -> f32 {
        self.i_q
            .get(2 * k + 1)
            .map_or(0.0, |v| f32::from(*v) / IQ_COUNTS_PER_UNIT)
    }

    /// The derived quadrature component of subcarrier `k`, in the core's
    /// working unit (raw counts divided by [`IQ_COUNTS_PER_UNIT`]).
    ///
    /// The sample is imaginary-first, so the quadrature byte of subcarrier `k`
    /// is the *first* byte of its pair (`i_q[2k]`).
    ///
    /// Returns `0.0` when `k` is out of range.
    #[must_use]
    pub fn q_at(&self, k: usize) -> f32 {
        self.i_q
            .get(2 * k)
            .map_or(0.0, |v| f32::from(*v) / IQ_COUNTS_PER_UNIT)
    }

    /// The derived per-subcarrier amplitude (`√(I² + Q²)`), linear and
    /// arbitrary, in ascending index order.
    #[must_use]
    pub fn amplitude(&self) -> Vec<f32> {
        (0..self.n_subcarriers())
            .map(|k| self.amplitude_at(k))
            .collect()
    }

    /// The derived amplitude of subcarrier `k` (`√(I² + Q²)`).
    #[must_use]
    pub fn amplitude_at(&self, k: usize) -> f32 {
        let i = self.i_at(k);
        let q = self.q_at(k);
        (i * i + q * q).sqrt()
    }

    /// The derived per-subcarrier phase (`atan2(Q, I)`), in radians, wrapped to
    /// `[-π, π]`. Per-frame phase is uncalibrated; cross-frame phase comparison
    /// is out of scope for v1.
    #[must_use]
    pub fn phase(&self) -> Vec<f32> {
        (0..self.n_subcarriers())
            .map(|k| self.phase_at(k))
            .collect()
    }

    /// The derived phase of subcarrier `k` (`atan2(Q, I)`), in radians.
    #[must_use]
    pub fn phase_at(&self, k: usize) -> f32 {
        self.q_at(k).atan2(self.i_at(k))
    }

    /// Validate the per-frame invariants that do not require neighbouring
    /// frames: a non-empty, even-length interleaved I/Q sample.
    ///
    /// The raw sample is already exact int8, so there is nothing to range-check
    /// here; amplitude and phase are finite for every possible sample.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.i_q.is_empty() {
            return Err(ValidationError::Length {
                field: "i_q",
                actual: 0,
                expected: 2,
            });
        }
        if !self.i_q.len().is_multiple_of(2) {
            return Err(ValidationError::Length {
                field: "i_q",
                actual: self.i_q.len(),
                expected: self.i_q.len() + 1,
            });
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
