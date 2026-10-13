//! The output `Sink` seam.
//!
//! `aquila-sensing` owns the seam; production adapters live in the edge crates
//! (`ChannelSink` in `aquila-api`, `HaSink` in `aquila-ha`) and the evaluation
//! adapter (`CollectorSink`) lives in `aquila-testkit`. The interface is one
//! method — emit an [`Event`] — plus an optional flush, so a fake is an ordinary
//! struct and the seam is real rather than speculative
//! (`architecture.md` §6, `interface-register.md`).

use aquila_types::Event;

use crate::error::SinkError;

/// The one write path out of the core.
pub trait Sink: Send {
    /// Emit one edge-triggered [`Event`].
    ///
    /// # Errors
    ///
    /// Returns a typed [`SinkError`] when the sink refuses the event, so a
    /// broken output stream is surfaced rather than swallowed.
    fn emit(&mut self, event: &Event) -> Result<(), SinkError>;

    /// Flush any buffered output. The default is a no-op.
    ///
    /// # Errors
    ///
    /// Returns a typed [`SinkError`] when the flush fails.
    fn flush(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
}
