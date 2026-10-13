//! `CollectorSink`: the evaluation adapter for the `Sink` seam.
//!
//! It captures every emitted [`Event`] for scoring while remaining a real
//! adapter behind the same trait the production sinks implement
//! (`testing.md` §1, `architecture.md` §6). The captured events are shared, so a
//! test can hand a clone to the core and keep reading after every `step`.

use std::sync::{Arc, Mutex};

use aquila_sensing::{Sink, SinkError};
use aquila_types::Event;

/// A sink that collects emitted events in memory.
#[derive(Debug, Clone, Default)]
pub struct CollectorSink {
    events: Arc<Mutex<Vec<Event>>>,
}

impl CollectorSink {
    /// Construct an empty collector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A shared handle to the captured events.
    ///
    /// Hand this to [`Sensing`](aquila_sensing::Sensing) (or clone the sink) and
    /// read it after every step.
    #[must_use]
    pub fn handle(&self) -> Arc<Mutex<Vec<Event>>> {
        Arc::clone(&self.events)
    }

    /// A snapshot of the events captured so far.
    #[must_use]
    pub fn events(&self) -> Vec<Event> {
        lock(&self.events).clone()
    }

    /// The number of events captured so far.
    #[must_use]
    pub fn len(&self) -> usize {
        lock(&self.events).len()
    }

    /// Whether no events have been captured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        lock(&self.events).is_empty()
    }
}

impl Sink for CollectorSink {
    fn emit(&mut self, event: &Event) -> Result<(), SinkError> {
        lock(&self.events).push(event.clone());
        Ok(())
    }
}

/// Lock a mutex, recovering from a poisoned lock rather than panicking.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
