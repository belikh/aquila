//! An advanceable clock for deterministic tests.
//!
//! The core never reads the wall clock; every arrival and every measurement
//! timestamp comes through the [`Clock`](aquila_types::Clock) seam. `FakeClock`
//! moves only when a test advances it, so a replay can pin the exact arrival of
//! every frame and a run is byte-identical across machines (`testing.md` §3).

use std::sync::atomic::{AtomicU64, Ordering};

use aquila_types::{Clock, CoreTime};

/// A manual, shared, advanceable clock.
#[derive(Debug, Default)]
pub struct FakeClock {
    nanos: AtomicU64,
}

impl FakeClock {
    /// Construct a clock at `nanos`.
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

    /// Set the clock to an absolute value.
    pub fn set(&self, nanos: u64) {
        self.nanos.store(nanos, Ordering::SeqCst);
    }

    /// The current value in nanoseconds.
    #[must_use]
    pub fn now_ns(&self) -> u64 {
        self.nanos.load(Ordering::SeqCst)
    }
}

impl Clock for FakeClock {
    fn now(&self) -> CoreTime {
        CoreTime(self.nanos.load(Ordering::SeqCst))
    }
}
