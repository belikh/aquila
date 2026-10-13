//! The `CsiSource` seam: the one place "where CSI comes from" is decided.
//!
//! The seam is deliberately small — one pull, one stop — so a fake is an
//! ordinary struct and tests need no network (`capture-and-firmware.md` §3.1).
//! It is satisfied by at least three real adapters: [`ReplaySource`]
//! (recorded), [`UdpLiveSource`](crate::UdpLiveSource) (live) and
//! `SimulatorSource` (`aquila-simulator`), so replay and live are genuinely
//! interchangeable and the seam is real, not speculative.
//!
//! [`ReplaySource`]: crate::ReplaySource
//!
//! The outbound half of the same protocol, [`NodeControl`](crate::NodeControl),
//! lives in this crate too: it is not a seventh seam (`ADR-0001` §3,
//! `interface-register.md`).

use aquila_types::CsiEnvelope;

use crate::error::SourceError;

/// One pull from a [`CsiSource`]: a decoded frame, or clean end-of-stream.
///
/// The frame is a [`CsiEnvelope`] (`data-contracts.md` §2): the pure node
/// [`CsiFrame`](aquila_types::CsiFrame) wrapped with host arrival data from the
/// [`Clock`](aquila_types::Clock) seam.
#[derive(Debug, Clone, PartialEq)]
pub enum FrameOrEnd {
    /// A decoded CSI frame with its host arrival metadata.
    Frame(CsiEnvelope),
    /// A finite source is exhausted. Live sources never yield this.
    End,
}

/// Pull-based, blocking, object-safe. One live implementation may yield frames
/// from many nodes; a replay implementation reads one Session.
pub trait CsiSource: Send {
    /// Blocking pull.
    ///
    /// Returns [`FrameOrEnd::Frame`] until a finite source is exhausted, then
    /// [`FrameOrEnd::End`]; returns [`SourceError::Stopped`] after
    /// [`shutdown`](CsiSource::shutdown).
    ///
    /// # Errors
    ///
    /// Returns a typed [`SourceError`] on I/O failure or an undecodable
    /// datagram that a source chooses not to tolerate.
    fn recv(&mut self) -> Result<FrameOrEnd, SourceError>;

    /// Request stop; unblocks a pending `recv`.
    ///
    /// Idempotent and safe from any thread that can reach the source.
    fn shutdown(&mut self);
}
