//! A virtual ESP32-S3 node: the live-path counterpart of `ReplaySource`.
//!
//! [`VirtualNode`] drives [`aquila_simulator`] and transmits **real** `AQCI`
//! datagrams over loopback UDP to a [`UdpLiveSource`](aquila_sources::UdpLiveSource),
//! and it serves the `AQCC` control channel (`capture-and-firmware.md` §6) so a
//! test can drive a full pairing/config/status/time-sync round trip. It is the
//! "where CSI comes from" fixture the Slice-7 tracer needs
//! (`roadmap.md`, Slice 7) and it exercises the same wire codec the firmware
//! does, so it cannot drift from live.
//!
//! The node is a dumb sensor: it frames CSI, assigns `sequence` and a
//! free-running device timestamp, and performs no DSP, inference or clock
//! discipline (`capture-and-firmware.md` §9.2). Its device clock is a virtual
//! monotonic counter advanced one tick per frame; the host (test) owns the real
//! time base through the injected `Clock`, exactly as the firmware never owns
//! the core clock.

use std::fmt;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use aquila_simulator::{SceneScript, SimConfig, SimulatorSource};
use aquila_sources::{CsiSource, FrameOrEnd, SourceError};
use aquila_types::{CsiFrame, MacAddr, NodeId};
use aquila_wire::{
    decode_control, encode_control, encode_datagram, result_message, AckStatus, ControlBody,
    ControlMessage, JoinAck, JoinRequest, Status, SyncEcho,
};

/// The read timeout used when polling the control socket, so a caller can
/// service the control channel without blocking forever.
const CONTROL_POLL_TIMEOUT: Duration = Duration::from_millis(250);

/// The immutable wiring of a [`VirtualNode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualNodeConfig {
    /// The dense node identity carried on the CSI data plane.
    pub node: NodeId,
    /// The factory base MAC carried on the control plane.
    pub mac: MacAddr,
    /// The node's boot epoch (`u16`, matching the CSI contract frame).
    pub boot: u16,
    /// The short-lived pairing code the node presents at join.
    pub pairing_code: u32,
    /// The core's CSI data-plane address (`9100/udp`).
    pub core_data: SocketAddr,
    /// The core's node control-plane address (`9101/udp`).
    pub core_control: SocketAddr,
    /// The 802.11 channel the node is configured to sense.
    pub channel: u8,
    /// The channel width in MHz.
    pub bandwidth_mhz: u8,
    /// The heartbeat period the node reports.
    pub heartbeat_ms: u32,
    /// The transmit-rate cap the node reports.
    pub max_tx_rate_hz: u32,
}

impl VirtualNodeConfig {
    /// Build a configuration with the v1 defaults for channel, heartbeat and
    /// rate cap.
    #[must_use]
    pub fn new(
        node: NodeId,
        mac: MacAddr,
        boot: u16,
        pairing_code: u32,
        core_data: SocketAddr,
        core_control: SocketAddr,
    ) -> Self {
        Self {
            node,
            mac,
            boot,
            pairing_code,
            core_data,
            core_control,
            channel: 6,
            bandwidth_mhz: 20,
            heartbeat_ms: 5_000,
            max_tx_rate_hz: 200,
        }
    }
}

/// The failure surface of a [`VirtualNode`].
#[derive(Debug)]
pub enum VirtualNodeError {
    /// The scene failed simulation.
    Sim(String),
    /// A source pull failed.
    Source(SourceError),
    /// A control datagram could not be encoded or decoded.
    Wire(aquila_wire::WireError),
    /// A socket operation failed.
    Io(std::io::Error),
}

impl fmt::Display for VirtualNodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sim(reason) => write!(f, "simulator error: {reason}"),
            Self::Source(error) => write!(f, "source error: {error}"),
            Self::Wire(error) => write!(f, "wire error: {error}"),
            Self::Io(error) => write!(f, "virtual node I/O error: {error}"),
        }
    }
}

impl std::error::Error for VirtualNodeError {}

impl From<SourceError> for VirtualNodeError {
    fn from(error: SourceError) -> Self {
        Self::Source(error)
    }
}

impl From<aquila_wire::WireError> for VirtualNodeError {
    fn from(error: aquila_wire::WireError) -> Self {
        Self::Wire(error)
    }
}

impl From<std::io::Error> for VirtualNodeError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// A virtual ESP32-S3 node streaming generated CSI over loopback UDP.
pub struct VirtualNode {
    config: VirtualNodeConfig,
    source: Box<dyn CsiSource>,
    data: UdpSocket,
    control: UdpSocket,
    buffer: Vec<u8>,
    sequence: u64,
    device_us: u64,
    tick_period_us: u64,
    next_request_id: u32,
    paired: bool,
    token: [u8; 32],
    config_version: u32,
    config_hash: [u8; 32],
    csi_enabled: bool,
    last_sequence: u64,
    dropped: u32,
}

impl VirtualNode {
    /// Build a virtual node for `script` and `sim_config`.
    ///
    /// The data and control sockets are bound to ephemeral loopback ports; the
    /// core's addresses come from `config`.
    ///
    /// # Errors
    ///
    /// Returns [`VirtualNodeError`] if the scene is invalid or a socket cannot
    /// be bound.
    pub fn new(
        script: SceneScript,
        sim_config: SimConfig,
        config: VirtualNodeConfig,
    ) -> Result<Self, VirtualNodeError> {
        let tick_period_us = if script.tick_hz > 0.0 {
            (1_000_000.0 / script.tick_hz).round().max(1.0) as u64
        } else {
            1
        };
        let source = SimulatorSource::new(script, sim_config)
            .map_err(|error| VirtualNodeError::Sim(error.to_string()))?;
        let data = UdpSocket::bind("127.0.0.1:0")?;
        let control = UdpSocket::bind("127.0.0.1:0")?;
        control.set_read_timeout(Some(CONTROL_POLL_TIMEOUT))?;
        Ok(Self {
            config,
            source: Box::new(source),
            data,
            control,
            buffer: vec![0u8; aquila_wire::MAX_DATAGRAM_LEN],
            sequence: 0,
            device_us: 0,
            tick_period_us,
            next_request_id: 1,
            paired: false,
            token: [0u8; 32],
            config_version: 0,
            config_hash: [0u8; 32],
            csi_enabled: true,
            last_sequence: 0,
            dropped: 0,
        })
    }

    /// The configured node identity.
    #[must_use]
    pub const fn node(&self) -> NodeId {
        self.config.node
    }

    /// The factory MAC of the node.
    #[must_use]
    pub const fn mac(&self) -> MacAddr {
        self.config.mac
    }

    /// The node's control-plane address.
    ///
    /// # Errors
    ///
    /// Returns [`VirtualNodeError::Io`] if the socket has no local address.
    pub fn control_addr(&self) -> Result<SocketAddr, VirtualNodeError> {
        Ok(self.control.local_addr()?)
    }

    /// Whether the node has completed pairing.
    #[must_use]
    pub const fn is_paired(&self) -> bool {
        self.paired
    }

    /// The pairing token issued by the core, zero before pairing.
    #[must_use]
    pub const fn token(&self) -> [u8; 32] {
        self.token
    }

    /// The active config version.
    #[must_use]
    pub const fn config_version(&self) -> u32 {
        self.config_version
    }

    /// Whether CSI streaming is enabled.
    #[must_use]
    pub const fn csi_enabled(&self) -> bool {
        self.csi_enabled
    }

    /// The last `sequence` the node assigned.
    #[must_use]
    pub const fn last_sequence(&self) -> u64 {
        self.last_sequence
    }

    /// The next datagram from the generator for this node, framed and sent.
    ///
    /// Frames for other nodes in the scene are skipped. The node stamps its own
    /// per-boot `sequence` and free-running device timestamp before encoding
    /// (`capture-and-firmware.md` §9.1). Returns `None` at end of stream.
    ///
    /// # Errors
    ///
    /// Returns [`VirtualNodeError`] on a source, encode or socket failure.
    pub fn send_next(&mut self) -> Result<Option<CsiFrame>, VirtualNodeError> {
        loop {
            match self.source.recv()? {
                FrameOrEnd::End => return Ok(None),
                FrameOrEnd::Frame(envelope) => {
                    if envelope.frame.node_id != self.config.node {
                        continue;
                    }
                    let mut frame = envelope.frame;
                    frame.node_id = self.config.node;
                    frame.boot_epoch = self.config.boot;
                    frame.sequence = self.sequence;
                    frame.device_time_us = self.device_us;
                    let datagram = encode_datagram(&frame);
                    self.data.send_to(&datagram, self.config.core_data)?;
                    self.sequence += 1;
                    self.device_us = self.device_us.saturating_add(self.tick_period_us);
                    self.last_sequence = frame.sequence;
                    return Ok(Some(frame));
                }
            }
        }
    }

    /// Send a `JOIN_REQUEST` to the core, returning its `request_id`.
    ///
    /// The pairing code is the one supplied in the configuration; a successful
    /// `JOIN_ACK` is stored when [`serve_control_once`](Self::serve_control_once)
    /// receives it.
    ///
    /// # Errors
    ///
    /// Returns [`VirtualNodeError`] on an encode or socket failure.
    pub fn send_join_request(&mut self) -> Result<u32, VirtualNodeError> {
        let request_id = self.alloc_request_id();
        let message = ControlMessage {
            request_id,
            body: ControlBody::JoinRequest(JoinRequest {
                node: self.config.mac,
                boot: self.config.boot,
                pairing_code: self.config.pairing_code,
                fw_version: concat!("aquila-testkit/", env!("CARGO_PKG_VERSION")).to_string(),
            }),
        };
        self.control
            .send_to(&encode_control(&message)?, self.config.core_control)?;
        Ok(request_id)
    }

    /// Send an unsolicited `STATUS` heartbeat to the core.
    ///
    /// # Errors
    ///
    /// Returns [`VirtualNodeError`] on an encode or socket failure.
    pub fn send_status(&mut self) -> Result<u32, VirtualNodeError> {
        let request_id = self.alloc_request_id();
        let message = ControlMessage {
            request_id,
            body: ControlBody::Status(Status {
                uptime_ms: self.device_us / 1_000,
                boot: self.config.boot,
                last_sequence: self.last_sequence,
                dropped: self.dropped,
                ring_depth: 0,
                temperature_c_x100: 2_500,
                config_version: self.config_version,
                last_rtt_us: 0,
            }),
        };
        self.control
            .send_to(&encode_control(&message)?, self.config.core_control)?;
        Ok(request_id)
    }

    /// Receive and answer at most one control message.
    ///
    /// Returns `Ok(true)` when a message was handled and `Ok(false)` when the
    /// poll timed out, so a caller can service the channel without blocking.
    ///
    /// # Errors
    ///
    /// Returns [`VirtualNodeError`] on a decode or socket failure.
    pub fn serve_control_once(&mut self) -> Result<bool, VirtualNodeError> {
        match self.control.recv_from(&mut self.buffer) {
            Ok((len, from)) => {
                let message = decode_control(&self.buffer[..len])?;
                self.handle(message, from)?;
                Ok(true)
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                Ok(false)
            }
            Err(e) => Err(VirtualNodeError::Io(e)),
        }
    }

    /// Service the control channel until it is idle.
    ///
    /// # Errors
    ///
    /// As [`serve_control_once`](Self::serve_control_once).
    pub fn drain_control(&mut self) -> Result<usize, VirtualNodeError> {
        let mut handled = 0usize;
        while self.serve_control_once()? {
            handled += 1;
        }
        Ok(handled)
    }

    /// Answer one decoded control message.
    fn handle(
        &mut self,
        message: ControlMessage,
        from: SocketAddr,
    ) -> Result<(), VirtualNodeError> {
        match message.body {
            ControlBody::JoinRequest(body) => {
                let reply = if body.pairing_code == self.config.pairing_code
                    && body.node == self.config.mac
                {
                    self.paired = true;
                    ControlMessage {
                        request_id: message.request_id,
                        body: ControlBody::JoinAck(JoinAck {
                            node: self.config.mac,
                            token: self.token,
                            config_version: self.config_version,
                            config_hash: self.config_hash,
                        }),
                    }
                } else {
                    result_message(
                        message.request_id,
                        AckStatus::Rejected,
                        "pairing code or node mismatch",
                    )
                };
                self.send_reply(from, &reply)?;
            }
            ControlBody::JoinAck(body) => {
                self.token = body.token;
                self.paired = true;
                self.config_version = body.config_version;
                self.config_hash = body.config_hash;
            }
            ControlBody::GetConfig => {
                let reply = result_message(
                    message.request_id,
                    AckStatus::Active,
                    format!("config_version={}", self.config_version),
                );
                self.send_reply(from, &reply)?;
            }
            ControlBody::SetConfig(body) => {
                let reply = if body.config_version < self.config_version {
                    result_message(
                        message.request_id,
                        AckStatus::Rejected,
                        "stale config version",
                    )
                } else {
                    self.config_version = body.config_version;
                    self.config_hash = body.config_hash;
                    self.csi_enabled = body.csi_enable;
                    result_message(message.request_id, AckStatus::Active, "config applied")
                };
                self.send_reply(from, &reply)?;
            }
            ControlBody::SetCsi(body) => {
                self.csi_enabled = body.enable;
                let reply = result_message(message.request_id, AckStatus::Active, "csi toggled");
                self.send_reply(from, &reply)?;
            }
            ControlBody::SyncPing(body) => {
                let echo = ControlMessage {
                    request_id: message.request_id,
                    body: ControlBody::SyncEcho(SyncEcho {
                        t1_us: body.t1_us,
                        t2_us: self.device_us,
                        t3_us: self.device_us,
                    }),
                };
                self.send_reply(from, &echo)?;
            }
            ControlBody::Reboot | ControlBody::Identify | ControlBody::FactoryReset => {
                let reply = result_message(message.request_id, AckStatus::Active, "accepted");
                self.send_reply(from, &reply)?;
            }
            // Replies and unsolicited messages from the core are not requests;
            // the node has nothing to answer.
            ControlBody::Result(_)
            | ControlBody::Status(_)
            | ControlBody::SyncEcho(_)
            | ControlBody::Event(_) => {}
        }
        Ok(())
    }

    /// Send one control reply to `to`.
    fn send_reply(&self, to: SocketAddr, message: &ControlMessage) -> Result<(), VirtualNodeError> {
        self.control.send_to(&encode_control(message)?, to)?;
        Ok(())
    }

    /// Allocate the next non-zero request id.
    fn alloc_request_id(&mut self) -> u32 {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        id
    }
}
