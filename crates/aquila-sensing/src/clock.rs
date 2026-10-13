//! The concrete `Clock` adapters.
//!
//! The `Clock` *trait* and the `CoreTime` newtype are declared in
//! `aquila-types` (ADR-0003 §5); every implementation lives here, in the crate
//! that owns the sync policy. [`SystemClock`] is the production adapter — a
//! monotonic host clock that must never jump or step. [`ManualClock`] is an
//! advanceable adapter for the crate's own tests; the replay and simulation
//! adapters inject their own clocks through the same seam.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use aquila_types::{Clock, CoreTime};

/// The production clock: a monotonic host clock, in nanoseconds since the
/// process started serving.
///
/// It wraps [`Instant`], so it is monotonic and never jumps. It is deliberately
/// **not** a wall clock: no NTP step and no leap second can move it.
#[derive(Debug)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    /// Anchor a new system clock at the current instant.
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> CoreTime {
        let nanos = self.origin.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        CoreTime(nanos)
    }
}

/// An advanceable clock for deterministic tests.
///
/// The time only moves when [`advance`](ManualClock::advance) or
/// [`set`](ManualClock::set) is called, so a test can pin the exact arrival of
/// every frame. It is safe to share behind an `Arc` because the counter is
/// atomic.
#[derive(Debug, Default)]
pub struct ManualClock {
    nanos: AtomicU64,
}

impl ManualClock {
    /// Construct a manual clock at `nanos`.
    #[must_use]
    pub fn new(nanos: u64) -> Self {
        Self {
            nanos: AtomicU64::new(nanos),
        }
    }

    /// Advance the clock by `delta_ns`, saturating at `u64::MAX`.
    pub fn advance(&self, delta_ns: u64) {
        self.nanos.fetch_add(delta_ns, Ordering::SeqCst);
    }

    /// Set the clock to an absolute value. It must not move backwards for a
    /// monotonic consumer; callers are responsible for that.
    pub fn set(&self, nanos: u64) {
        self.nanos.store(nanos, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> CoreTime {
        CoreTime(self.nanos.load(Ordering::SeqCst))
    }
}
