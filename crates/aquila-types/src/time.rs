//! Time: the two clocks, the span and window types, and the [`Clock`] seam.
//!
//! `aquila-types` declares the one-method [`Clock`] trait and the
//! [`CoreTime`]/[`DeviceTime`] newtypes so that `aquila-sources` and
//! `aquila-simulator` can inject an `Arc<dyn Clock>` for deterministic replay
//! without an upward dependency edge (ADR-0003 §5). Every *implementation* of
//! the trait — `SystemClock`, `ReplayClock`, the drift estimator — lives in
//! `aquila-sensing`. `SystemClock` is deliberately **not** in this crate.

use serde::{Deserialize, Serialize};

use crate::error::{invariant, ValidationError};

/// Monotonic core time, in nanoseconds since core boot.
///
/// This is the single reference clock: all inference, windows and ledger
/// entries are expressed in it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct CoreTime(pub u64);

impl CoreTime {
    /// Construct a core timestamp from nanoseconds.
    #[must_use]
    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    /// The timestamp in nanoseconds.
    #[must_use]
    pub const fn as_nanos(&self) -> u64 {
        self.0
    }
}

/// Node-local device time, in nanoseconds from the node's `esp_timer`.
///
/// Cheap, uncorrected, and drifting relative to the core. Nodes stamp every CSI
/// frame with their own device time and never adjust it; the core owns
/// correction.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct DeviceTime(pub u64);

impl DeviceTime {
    /// Construct a device timestamp from nanoseconds.
    #[must_use]
    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    /// The timestamp in nanoseconds.
    #[must_use]
    pub const fn as_nanos(&self) -> u64 {
        self.0
    }
}

/// The injectable core time source: the core's source of "now".
///
/// The one method returns a [`CoreTime`]. A production adapter wraps a
/// monotonic host clock and must neither jump nor step; a replay adapter
/// advances on command for byte-identical tests. Both adapters live in
/// `aquila-sensing` (ADR-0003 §5), which is why only the trait is declared
/// here.
pub trait Clock: Send + Sync {
    /// The current core time.
    fn now(&self) -> CoreTime;
}

/// A half-open time span on the host monotonic clock, then wall time.
///
/// Stored both as nanoseconds on the host monotonic clock and as Unix
/// milliseconds so that audit and replay can each read the field they need.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeSpan {
    /// Start on the host monotonic clock, in nanoseconds.
    pub start_mono_ns: u64,
    /// End on the host monotonic clock, in nanoseconds. Invariant:
    /// `end_mono_ns > start_mono_ns`.
    pub end_mono_ns: u64,
    /// Start as Unix wall time, in milliseconds.
    pub start_unix_ms: i64,
    /// End as Unix wall time, in milliseconds.
    pub end_unix_ms: i64,
}

impl TimeSpan {
    /// Validate the half-openness invariant.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.end_mono_ns > self.start_mono_ns {
            Ok(())
        } else {
            Err(invariant("TimeSpan requires end_mono_ns > start_mono_ns"))
        }
    }

    /// The span's duration on the monotonic clock, in nanoseconds.
    ///
    /// Returns `0` when the span is malformed rather than panicking; callers
    /// should [`validate`](Self::validate) first.
    #[must_use]
    pub const fn duration_mono_ns(&self) -> u64 {
        self.end_mono_ns.saturating_sub(self.start_mono_ns)
    }
}

/// A core-time aligned, half-open window `[start, end)` over a [`CoreTime`]
/// axis.
///
/// This is the window form used by fusion and feature extraction
/// (`fusion-sync.md` §2.1): a window over the single core reference clock,
/// distinct from a [`TimeSpan`], which also carries wall time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeWindow {
    /// The inclusive start of the window.
    pub start: CoreTime,
    /// The exclusive end of the window. Invariant: `end > start`.
    pub end: CoreTime,
}

impl TimeWindow {
    /// Construct a window, rejecting an empty or inverted interval.
    pub fn new(start: CoreTime, end: CoreTime) -> Result<Self, ValidationError> {
        let window = Self { start, end };
        window.validate()?;
        Ok(window)
    }

    /// Validate that the window is non-empty.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.end.0 > self.start.0 {
            Ok(())
        } else {
            Err(invariant("TimeWindow requires end > start"))
        }
    }

    /// Whether a core timestamp falls within the half-open window.
    #[must_use]
    pub const fn contains(&self, t: CoreTime) -> bool {
        t.0 >= self.start.0 && t.0 < self.end.0
    }

    /// The window's duration, in nanoseconds.
    #[must_use]
    pub const fn duration_ns(&self) -> u64 {
        self.end.0.saturating_sub(self.start.0)
    }
}
