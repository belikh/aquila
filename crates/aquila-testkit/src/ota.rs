//! A deterministic fake firmware node and transport for the OTA tracer
//! (ticket #13).
//!
//! [`FakeOtaNode`] is a node-side OTA agent with no hardware: it verifies each
//! chunk's CRC-32, writes chunks sequentially, answers cumulative `OTA_ACK`s,
//! can drop a named chunk once (to force a retransmission), and can go silent at
//! a chosen offset (to force a resumable interruption). [`FakeOtaTransport`] is
//! the core-side transport that carries the manager's messages to the node and
//! queues its replies, implementing the same [`OtaTransport`] seam the UDP
//! production path uses.
//!
//! The fake is honest about what it is: a simulated node, so any measurement it
//! produces is synthetic, never hardware evidence.

use std::collections::{BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};

use aquila_api::{OtaError, OtaTransport};
use aquila_types::sha256;
use aquila_wire::{
    ControlBody, ControlMessage, OtaAck, OtaAckStatus, OtaOutcome, OtaResult, OtaState, OtaStatus,
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

/// A deterministic signing key from a fixed 32-byte seed.
#[must_use]
pub fn signing_key(seed: [u8; 32]) -> SigningKey {
    SigningKey::from_bytes(&seed)
}

/// The Ed25519 signature over an image's SHA-256 digest (`firmware-ota.md` §4).
#[must_use]
pub fn sign_image(image: &[u8], key: &SigningKey) -> [u8; 64] {
    let digest = sha256(image);
    key.sign(&digest).to_bytes()
}

/// The trusted verification key for a signing key.
#[must_use]
pub fn verifying_key(key: &SigningKey) -> VerifyingKey {
    key.verifying_key()
}

/// Recompute and verify an Ed25519 signature over an image digest.
#[must_use]
pub fn verify_image(image: &[u8], signature: &[u8; 64], key: &VerifyingKey) -> bool {
    key.verify_strict(&sha256(image), &Signature::from_bytes(signature))
        .is_ok()
}

/// A fake ESP32-S3 node's OTA agent, sharing state across reconnects.
#[derive(Debug)]
pub struct FakeOtaNode {
    /// The expected whole-image digest.
    expected: [u8; 32],
    /// The bytes received so far, written strictly sequentially.
    received: Vec<u8>,
    /// Chunk offsets to drop the first time they are offered.
    drop_once: BTreeSet<u32>,
    /// Chunk offsets already dropped once.
    dropped: BTreeSet<u32>,
    /// Go silent once this many bytes have been received.
    interrupt_at: Option<u32>,
    /// Whether the node has gone silent.
    interrupted: bool,
    /// The node's reported state.
    state: OtaState,
}

impl FakeOtaNode {
    /// A fake node expecting an image with digest `expected`.
    #[must_use]
    pub fn new(expected: [u8; 32]) -> Self {
        Self {
            expected,
            received: Vec::new(),
            drop_once: BTreeSet::new(),
            dropped: BTreeSet::new(),
            interrupt_at: None,
            interrupted: false,
            state: OtaState::Idle,
        }
    }

    /// Drop the chunk at `offset` the first time it is offered.
    pub fn drop_chunk_once(&mut self, offset: u32) {
        self.drop_once.insert(offset);
    }

    /// Go silent (simulate link loss) once `bytes` have been received.
    pub fn interrupt_after(&mut self, bytes: u32) {
        self.interrupt_at = Some(bytes);
    }

    /// Clear the interruption and let the node answer again (a resumed link).
    pub fn resume(&mut self) {
        self.interrupted = false;
        self.interrupt_at = None;
    }

    /// The cumulative bytes received, the resumable offset.
    #[must_use]
    pub fn received_upto(&self) -> u32 {
        self.received.len() as u32
    }

    /// The node's current OTA state.
    #[must_use]
    pub fn state(&self) -> OtaState {
        self.state
    }

    /// Whether the whole image has been received and its digest verified.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        sha256(&self.received) == self.expected
    }

    /// Handle one core message, returning the node's reply, if any.
    fn handle(&mut self, message: &ControlMessage) -> Option<ControlMessage> {
        let request_id = message.request_id;
        match &message.body {
            ControlBody::OtaBegin(begin) => {
                self.state = OtaState::Receiving;
                // A fresh transfer for a different image resets any stale bytes.
                if begin.sha256 != self.expected {
                    self.received.clear();
                }
                Some(self.ack(request_id, OtaAckStatus::Ok))
            }
            ControlBody::OtaChunk(chunk) => {
                if self.interrupted {
                    return None;
                }
                let offset = chunk.offset as usize;
                if offset == self.received.len() {
                    if self.drop_once.remove(&chunk.offset) {
                        self.dropped.insert(chunk.offset);
                        // A dropped chunk is not written; ACK the unchanged
                        // cumulative offset so the core retransmits.
                    } else if crc32fast::hash(&chunk.bytes) == chunk.crc32 {
                        self.received.extend_from_slice(&chunk.bytes);
                    }
                }
                if let Some(limit) = self.interrupt_at {
                    if self.received.len() as u32 >= limit {
                        self.interrupted = true;
                        return None;
                    }
                }
                Some(self.ack(request_id, OtaAckStatus::Ok))
            }
            ControlBody::OtaEnd(_end) => {
                if self.interrupted {
                    return None;
                }
                let complete = sha256(&self.received) == self.expected;
                let (outcome, reason) = if complete {
                    self.state = OtaState::Activating;
                    (OtaOutcome::Activated, String::new())
                } else {
                    self.state = OtaState::Failed;
                    (OtaOutcome::Failed, "image incomplete".to_string())
                };
                Some(ControlMessage {
                    request_id,
                    body: ControlBody::OtaResult(OtaResult { outcome, reason }),
                })
            }
            ControlBody::OtaAbort(_) => {
                self.state = OtaState::Failed;
                None
            }
            ControlBody::OtaStatus(_) => Some(ControlMessage {
                request_id,
                body: ControlBody::OtaStatus(OtaStatus {
                    state: self.state,
                    received_upto: self.received.len() as u32,
                    size: self.received.len() as u32,
                }),
            }),
            _ => None,
        }
    }

    /// A cumulative ACK at the current offset.
    fn ack(&self, request_id: u32, status: OtaAckStatus) -> ControlMessage {
        ControlMessage {
            request_id,
            body: ControlBody::OtaAck(OtaAck {
                received_upto: self.received.len() as u32,
                status,
            }),
        }
    }
}

/// The core-side transport over a [`FakeOtaNode`].
pub struct FakeOtaTransport {
    node: Arc<Mutex<FakeOtaNode>>,
    to_core: VecDeque<ControlMessage>,
}

impl FakeOtaTransport {
    /// A transport delivering to `node`.
    #[must_use]
    pub fn new(node: Arc<Mutex<FakeOtaNode>>) -> Self {
        Self {
            node,
            to_core: VecDeque::new(),
        }
    }
}

impl OtaTransport for FakeOtaTransport {
    fn send(&mut self, message: &ControlMessage) -> Result<(), OtaError> {
        let reply = {
            let mut node = lock(&self.node);
            node.handle(message)
        };
        if let Some(reply) = reply {
            self.to_core.push_back(reply);
        }
        Ok(())
    }

    fn try_recv(&mut self) -> Result<Option<ControlMessage>, OtaError> {
        Ok(self.to_core.pop_front())
    }
}

/// Lock a mutex, recovering from a poisoned lock rather than panicking.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aquila_api::{FirmwareCompat, FirmwareStore, ImportRequest, Ota, OtaConfig, OtaManager};

    /// A deterministic image of `size` bytes.
    fn image(size: usize) -> Vec<u8> {
        (0..size).map(|index| (index % 251) as u8).collect()
    }

    /// A store holding a signed image, returning the store and the manifest id.
    fn store_with_image(
        dir: &std::path::Path,
        bytes: &[u8],
        version: &str,
        seed: [u8; 32],
    ) -> (FirmwareStore, String, VerifyingKey) {
        let key = signing_key(seed);
        let store = FirmwareStore::open(dir, verifying_key(&key)).expect("open store");
        let manifest = store
            .import(
                FirmwareCompat::encode(1, 0),
                "esp32s3",
                ImportRequest {
                    image: bytes.to_vec(),
                    signature: sign_image(bytes, &key),
                    version: version.to_string(),
                    target: "esp32s3".to_string(),
                    compatibility: FirmwareCompat::new(
                        FirmwareCompat::encode(1, 0),
                        FirmwareCompat::encode(1, 9),
                    ),
                    notes: "tracer".to_string(),
                },
            )
            .expect("import");
        let id = manifest.id().to_string();
        (store, id, verifying_key(&key))
    }

    #[test]
    fn a_full_image_transfers_and_activates() {
        let dir = std::env::temp_dir().join(format!("aquila-ota-full-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let bytes = image(1024);
        let (store, id, _key) = store_with_image(&dir, &bytes, "1.0.0", [7u8; 32]);
        let manifest = store.manifest(&id).expect("manifest");

        let node = Arc::new(Mutex::new(FakeOtaNode::new(sha256(&bytes))));
        let mut transport = FakeOtaTransport::new(Arc::clone(&node));
        let manager = OtaManager::new(OtaConfig {
            chunk_size: 128,
            window: 4,
            max_retries: 6,
            max_concurrent: 1,
        });
        let stats = manager
            .transfer(&bytes, &manifest, &mut transport)
            .expect("transfer");
        assert_eq!(stats.outcome, OtaOutcome::Activated);
        assert_eq!(stats.retransmissions, 0);
        assert!(lock(&node).is_complete());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_dropped_chunk_is_retransmitted() {
        let dir = std::env::temp_dir().join(format!("aquila-ota-drop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let bytes = image(1024);
        let (store, id, _key) = store_with_image(&dir, &bytes, "1.0.0", [8u8; 32]);
        let manifest = store.manifest(&id).expect("manifest");

        let node = Arc::new(Mutex::new(FakeOtaNode::new(sha256(&bytes))));
        lock(&node).drop_chunk_once(128);
        let mut transport = FakeOtaTransport::new(Arc::clone(&node));
        let manager = OtaManager::new(OtaConfig {
            chunk_size: 128,
            window: 4,
            max_retries: 6,
            max_concurrent: 1,
        });
        let stats = manager
            .transfer(&bytes, &manifest, &mut transport)
            .expect("transfer");
        assert_eq!(stats.outcome, OtaOutcome::Activated);
        assert!(
            stats.retransmissions >= 1,
            "the dropped chunk must be resent"
        );
        assert!(lock(&node).is_complete());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_interrupted_transfer_resumes_from_the_node_offset() {
        let dir = std::env::temp_dir().join(format!("aquila-ota-resume-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let bytes = image(2048);
        let (_store, id, key) = store_with_image(&dir, &bytes, "1.0.0", [9u8; 32]);
        let ota = Ota::with_store(FirmwareStore::open(&dir, key).expect("reopen store"));
        let node = Arc::new(Mutex::new(FakeOtaNode::new(sha256(&bytes))));
        lock(&node).interrupt_after(512);

        // First attempt: the link drops, so the transfer stalls.
        ota.register_transport(
            "aa:bb:cc:dd:ee:ff",
            Box::new(FakeOtaTransport::new(Arc::clone(&node))),
        );
        let failed = ota
            .install("aa:bb:cc:dd:ee:ff", "esp32s3", &id, false)
            .expect("job");
        assert_eq!(failed.state.as_str(), "failed");
        assert_eq!(failed.received_bytes, 0);
        assert!(lock(&node).received_upto() >= 512);

        // The link returns; a fresh transport reports the resumable offset.
        lock(&node).resume();
        ota.register_transport(
            "aa:bb:cc:dd:ee:ff",
            Box::new(FakeOtaTransport::new(Arc::clone(&node))),
        );
        let resumed = ota
            .install("aa:bb:cc:dd:ee:ff", "esp32s3", &id, false)
            .expect("job");
        assert_eq!(resumed.state.as_str(), "succeeded");
        assert_eq!(resumed.received_bytes, bytes.len() as u64);
        assert!(lock(&node).is_complete());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bad_signature_is_rejected_and_never_offered() {
        let dir = std::env::temp_dir().join(format!("aquila-ota-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let bytes = image(512);
        let key = signing_key([3u8; 32]);
        let store = FirmwareStore::open(&dir, verifying_key(&key)).expect("open");
        // Sign the wrong bytes: a valid-length but invalid signature.
        let mut signature = sign_image(b"not the image", &key);
        signature[0] ^= 0xFF;
        let result = store.import(
            FirmwareCompat::encode(1, 0),
            "esp32s3",
            ImportRequest {
                image: bytes,
                signature,
                version: "1.0.0".to_string(),
                target: "esp32s3".to_string(),
                compatibility: FirmwareCompat::new(
                    FirmwareCompat::encode(1, 0),
                    FirmwareCompat::encode(1, 9),
                ),
                notes: String::new(),
            },
        );
        assert!(matches!(result, Err(aquila_api::StoreError::BadSignature)));
        assert!(
            store.list().is_empty(),
            "a rejected image must never be offered"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The tracer's ledger rows are recorded (record-and-diff policy; gating is
    /// deliberately off, `measurement.md` §5).
    #[test]
    fn the_tracer_records_ledger_rows() {
        let dir = std::env::temp_dir().join(format!("aquila-ota-ledger-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let bytes = image(256);
        let key = signing_key([4u8; 32]);
        let store = FirmwareStore::open(&dir, verifying_key(&key)).expect("open");
        let manifest = store
            .import(
                FirmwareCompat::encode(1, 0),
                "esp32s3",
                ImportRequest {
                    image: bytes.clone(),
                    signature: sign_image(&bytes, &key),
                    version: "2.0.0".to_string(),
                    target: "esp32s3".to_string(),
                    compatibility: FirmwareCompat::new(
                        FirmwareCompat::encode(1, 0),
                        FirmwareCompat::encode(1, 9),
                    ),
                    notes: String::new(),
                },
            )
            .expect("import");

        let recorder = crate::RecordingMetricSink::new();
        // A clone shares state with the sink the runtime owns.
        let ota = Ota::with_store(store);
        ota.set_metric_sink(Box::new(recorder.clone()));
        let node = Arc::new(Mutex::new(FakeOtaNode::new(sha256(&bytes))));
        ota.register_transport("aa:bb:cc:dd:ee:ff", Box::new(FakeOtaTransport::new(node)));
        let job = ota
            .install("aa:bb:cc:dd:ee:ff", "esp32s3", manifest.id(), false)
            .expect("job");
        assert_eq!(job.state.as_str(), "succeeded");

        let rows = recorder.records();
        assert!(rows
            .iter()
            .any(|row| row.name.as_str() == "firmware.ota.result"));
        assert!(rows
            .iter()
            .any(|row| row.name.as_str() == "firmware.version.installed"));
        // Record-and-diff: diffing the rows against themselves is a no-op.
        let deltas = aquila_metrics::diff(&rows, &rows);
        assert_eq!(deltas.len(), rows.len());
        assert!(deltas.iter().all(|delta| !delta.is_downgraded()));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
