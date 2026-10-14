//! The core OTA manager: chunk an image and drive a node over a transport
//! (`firmware-ota.md` §5).
//!
//! The manager is transport-agnostic: it speaks [`ControlMessage`]s through the
//! [`OtaTransport`] seam, so the production UDP transport and a deterministic
//! fake are the same code path. The transfer is a sliding window of chunks with
//! a cumulative acknowledgement, per-chunk CRC-32, bounded retransmission and a
//! node-driven resumable offset (the node's `OTA_ACK`/`OTA_STATUS` tells the
//! core where to resume).

use std::sync::Mutex;

use aquila_wire::{
    ControlBody, ControlMessage, OtaBegin, OtaChunk, OtaEnd, OtaOutcome, OtaResult,
    OTA_DEFAULT_CHUNK_SIZE, OTA_DEFAULT_WINDOW, OTA_MAX_CHUNK_SIZE,
};

use super::store::FirmwareManifest;

/// The transfer policy (`firmware-ota.md` §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaConfig {
    /// The chunk payload size in bytes.
    pub chunk_size: u16,
    /// The sliding-window depth in chunks.
    pub window: u8,
    /// Bounded retries before the manager gives up on a stalled transfer.
    pub max_retries: u32,
    /// The global cap on concurrent transfers.
    pub max_concurrent: usize,
}

impl Default for OtaConfig {
    fn default() -> Self {
        Self {
            chunk_size: OTA_DEFAULT_CHUNK_SIZE,
            window: OTA_DEFAULT_WINDOW,
            max_retries: 8,
            max_concurrent: 1,
        }
    }
}

impl OtaConfig {
    /// Clamp the chunk size to the datagram budget so a chunk always fits.
    #[must_use]
    pub fn sanctioned_chunk_size(&self) -> u16 {
        self.chunk_size.clamp(1, OTA_MAX_CHUNK_SIZE)
    }
}

/// Everything that can go wrong driving a transfer.
#[derive(Debug, thiserror::Error)]
pub enum OtaError {
    /// The control codec failed to encode a message.
    #[error("ota wire error: {0}")]
    Wire(#[from] aquila_wire::WireError),
    /// The transport failed to carry a message.
    #[error("ota transport error: {0}")]
    Transport(String),
    /// The node explicitly refused the transfer.
    #[error("node refused the ota transfer: {0}")]
    NodeRejected(String),
    /// The transfer stalled with no progress after the retry budget.
    #[error("ota transfer stalled after {0} retries")]
    Stalled(u32),
    /// The node reported a non-success outcome.
    #[error("ota outcome {outcome:?}: {reason}")]
    Outcome {
        /// The outcome the node reported.
        outcome: OtaOutcome,
        /// The node's reason.
        reason: String,
    },
    /// The image is larger than the wire `size` field can describe.
    #[error("image size {0} exceeds the 4 GiB transfer limit")]
    TooLarge(usize),
    /// The manifest is malformed for transfer.
    #[error("firmware manifest is not transferable: {0}")]
    Metadata(String),
    /// Another transfer holds the concurrency slot.
    #[error("ota concurrency cap reached")]
    Busy,
}

/// The transport seam a transfer drives (`firmware-ota.md` §5).
///
/// Production is a UDP `AQCC` endpoint; a test supplies a deterministic
/// in-memory fake. Both are the same code path.
pub trait OtaTransport: Send {
    /// Send one control message to the node.
    fn send(&mut self, message: &ControlMessage) -> Result<(), OtaError>;

    /// Take the next pending control message from the node, or `None` when none
    /// is immediately available.
    fn try_recv(&mut self) -> Result<Option<ControlMessage>, OtaError>;
}

/// The result of a completed transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaStats {
    /// The number of image bytes the core offered.
    pub sent_bytes: u64,
    /// The cumulative offset the node acknowledged.
    pub received_bytes: u64,
    /// How many times the window was retransmitted.
    pub retransmissions: u32,
    /// The node's final outcome.
    pub outcome: OtaOutcome,
}

/// The core OTA manager: one transfer orchestrator with a global concurrency
/// cap (`firmware-ota.md` §3, §5).
#[derive(Debug)]
pub struct OtaManager {
    config: OtaConfig,
    active: Mutex<usize>,
    next_transfer_id: Mutex<u32>,
}

impl OtaManager {
    /// Construct a manager with an explicit policy.
    #[must_use]
    pub fn new(config: OtaConfig) -> Self {
        Self {
            config,
            active: Mutex::new(0),
            next_transfer_id: Mutex::new(1),
        }
    }

    /// The configured policy.
    #[must_use]
    pub fn config(&self) -> &OtaConfig {
        &self.config
    }

    /// Allocate the next non-zero transfer id.
    #[must_use]
    pub fn allocate_transfer_id(&self) -> u32 {
        let mut id = lock(&self.next_transfer_id);
        let current = *id;
        *id = current.wrapping_add(1).max(1);
        current
    }

    /// Drive one full image to a node over `transport`, resuming wherever the
    /// node's cumulative acknowledgement says to start.
    ///
    /// # Errors
    ///
    /// Returns a typed [`OtaError`] when the transport fails, the node refuses,
    /// the outcome is not success, or the transfer stalls.
    pub fn transfer(
        &self,
        image: &[u8],
        manifest: &FirmwareManifest,
        transport: &mut dyn OtaTransport,
    ) -> Result<OtaStats, OtaError> {
        let _slot = self.acquire()?;
        if image.len() > u32::MAX as usize {
            return Err(OtaError::TooLarge(image.len()));
        }
        let size = image.len() as u32;
        let sha256 = manifest
            .sha256_bytes()
            .ok_or_else(|| OtaError::Metadata("bad sha256".to_string()))?;
        let signature = manifest
            .signature_bytes()
            .ok_or_else(|| OtaError::Metadata("bad signature".to_string()))?;

        let chunk_size = self.config.sanctioned_chunk_size();
        let window = u32::from(self.config.window.max(1));
        let transfer_id = self.allocate_transfer_id();

        // OTA_BEGIN, then wait for the node to report its resumable offset.
        let begin = ControlMessage {
            request_id: transfer_id,
            body: ControlBody::OtaBegin(OtaBegin {
                image_id: transfer_id,
                size,
                sha256,
                signature,
                version: manifest.version.clone(),
                chunk_size,
                window: self.config.window.max(1),
            }),
        };
        let mut base = self.wait_for_offset(&begin, transport, size)?;
        let mut next = base;
        let mut retries = 0u32;
        let mut retransmissions = 0u32;
        let mut ended = false;

        loop {
            while next < size && (next - base) < window * u32::from(chunk_size) {
                let end = next.saturating_add(u32::from(chunk_size)).min(size);
                let bytes = &image[next as usize..end as usize];
                let chunk = ControlMessage {
                    request_id: transfer_id,
                    body: ControlBody::OtaChunk(OtaChunk {
                        offset: next,
                        len: (end - next) as u16,
                        crc32: crc32fast::hash(bytes),
                        bytes: bytes.to_vec(),
                    }),
                };
                transport.send(&chunk)?;
                next = end;
            }

            if next >= size && !ended {
                transport.send(&ControlMessage {
                    request_id: transfer_id,
                    body: ControlBody::OtaEnd(OtaEnd {
                        image_id: transfer_id,
                        sha256,
                    }),
                })?;
                ended = true;
            }

            let mut progressed = false;
            while let Some(message) = transport.try_recv()? {
                match message.body {
                    ControlBody::OtaAck(ack) => {
                        let acked = ack.received_upto.min(size);
                        if acked > base {
                            base = acked;
                            progressed = true;
                        }
                        // A NACK (or any non-advancing ACK) leaves `base`
                        // unchanged, so the retransmit branch below resends the
                        // window from `base`.
                    }
                    ControlBody::OtaStatus(status) => {
                        let acked = status.received_upto.min(size);
                        if acked > base {
                            base = acked;
                            progressed = true;
                        }
                    }
                    ControlBody::OtaResult(result) => {
                        return self.finish(result, base, retransmissions);
                    }
                    ControlBody::OtaAbort(abort) => {
                        return Err(OtaError::NodeRejected(abort.reason));
                    }
                    _ => {}
                }
            }

            if progressed {
                retries = 0;
                if next < base {
                    next = base;
                }
                continue;
            }

            retries = retries.saturating_add(1);
            if retries > self.config.max_retries {
                return Err(OtaError::Stalled(retries));
            }
            if ended {
                transport.send(&ControlMessage {
                    request_id: transfer_id,
                    body: ControlBody::OtaEnd(OtaEnd {
                        image_id: transfer_id,
                        sha256,
                    }),
                })?;
            } else {
                next = base;
            }
            retransmissions = retransmissions.saturating_add(1);
        }
    }

    /// Send `OTA_BEGIN` and wait for the node's cumulative offset, retrying
    /// within the budget.
    fn wait_for_offset(
        &self,
        begin: &ControlMessage,
        transport: &mut dyn OtaTransport,
        size: u32,
    ) -> Result<u32, OtaError> {
        for _ in 0..=self.config.max_retries {
            transport.send(begin)?;
            while let Some(message) = transport.try_recv()? {
                match message.body {
                    ControlBody::OtaAck(ack) => return Ok(ack.received_upto.min(size)),
                    ControlBody::OtaStatus(status) => return Ok(status.received_upto.min(size)),
                    ControlBody::OtaResult(result) => {
                        return Err(self.result_error(result));
                    }
                    ControlBody::OtaAbort(abort) => {
                        return Err(OtaError::NodeRejected(abort.reason));
                    }
                    _ => {}
                }
            }
        }
        Err(OtaError::Stalled(self.config.max_retries + 1))
    }

    /// Map a node `OTA_RESULT` to the transfer result or an error.
    fn finish(
        &self,
        result: OtaResult,
        received_bytes: u32,
        retransmissions: u32,
    ) -> Result<OtaStats, OtaError> {
        if result.outcome == OtaOutcome::Activated || result.outcome == OtaOutcome::Verified {
            Ok(OtaStats {
                sent_bytes: u64::from(received_bytes),
                received_bytes: u64::from(received_bytes),
                retransmissions,
                outcome: result.outcome,
            })
        } else {
            Err(self.result_error(result))
        }
    }

    /// Convert a non-success `OTA_RESULT` into a typed error.
    fn result_error(&self, result: OtaResult) -> OtaError {
        OtaError::Outcome {
            outcome: result.outcome,
            reason: result.reason,
        }
    }

    /// Acquire the concurrency slot, or fail with [`OtaError::Busy`].
    fn acquire(&self) -> Result<SlotGuard<'_>, OtaError> {
        let mut active = lock(&self.active);
        if *active >= self.config.max_concurrent.max(1) {
            return Err(OtaError::Busy);
        }
        *active += 1;
        Ok(SlotGuard {
            active: &self.active,
        })
    }
}

/// A concurrency slot, released on drop.
struct SlotGuard<'a> {
    active: &'a Mutex<usize>,
}

impl Drop for SlotGuard<'_> {
    fn drop(&mut self) {
        let mut active = lock(self.active);
        *active = active.saturating_sub(1);
    }
}

/// Lock a mutex, recovering from a poisoned lock rather than panicking.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

