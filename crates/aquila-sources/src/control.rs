//! The outbound half of the node seam: the node control channel.
//!
//! The control plane is a separate UDP request/response protocol on its own
//! port (`capture-and-firmware.md` §6), core ↔ node. [`NodeControl`] is
//! **not** a seventh seam: it is the outbound half of the same `CsiSource`
//! protocol, in the same crate, with the same adapters (`ADR-0001` §3). It
//! encodes with [`aquila_wire::encode_control`] and decodes with
//! [`aquila_wire::decode_control`], so the control codec has one definition.

use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use aquila_wire::{decode_control, encode_control, ControlMessage, MAX_DATAGRAM_LEN};

use crate::error::SourceError;

/// Sends and receives `AQCC` control messages over UDP.
///
/// A control message is keyed by a caller nonce; a reply echoes it
/// (`capture-and-firmware.md` §6.1). A caller sends its request and matches the
/// reply by `request_id`.
#[derive(Debug)]
pub struct NodeControl {
    socket: UdpSocket,
    buffer: Vec<u8>,
}

impl NodeControl {
    /// Bind a control socket to `addr`.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Io`] if the address cannot be bound.
    pub fn bind(addr: SocketAddr) -> Result<Self, SourceError> {
        let socket = UdpSocket::bind(addr)?;
        Ok(Self {
            socket,
            buffer: vec![0u8; MAX_DATAGRAM_LEN],
        })
    }

    /// Bind an ephemeral control socket on loopback; useful in tests.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Io`] if the loopback address cannot be bound.
    pub fn bind_any() -> Result<Self, SourceError> {
        Self::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
    }

    /// The actual bound address.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Io`] if the socket has no local address.
    pub fn local_addr(&self) -> Result<SocketAddr, SourceError> {
        Ok(self.socket.local_addr()?)
    }

    /// Set the receive timeout; `None` blocks indefinitely.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Io`] if the option cannot be set.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<(), SourceError> {
        Ok(self.socket.set_read_timeout(timeout)?)
    }

    /// Encode and send one control message to `to`.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Decode`] if the message cannot be encoded, or
    /// [`SourceError::Io`] if the datagram cannot be sent.
    pub fn send(&self, to: SocketAddr, message: &ControlMessage) -> Result<(), SourceError> {
        let bytes = encode_control(message)?;
        self.socket.send_to(&bytes, to)?;
        Ok(())
    }

    /// Receive and decode one control message.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Io`] on a socket failure, or
    /// [`SourceError::Decode`] if the datagram is not a valid control message.
    pub fn recv(&mut self) -> Result<(ControlMessage, SocketAddr), SourceError> {
        let (len, from) = self.socket.recv_from(&mut self.buffer)?;
        let message = decode_control(&self.buffer[..len])?;
        Ok((message, from))
    }
}
