//! The core OTA subsystem: the signed image store and the transfer manager
//! (`firmware-ota.md` §3–§7, ticket #13).
//!
//! [`FirmwareStore`] verifies and holds signed images; [`OtaManager`] drives a
//! chunked transfer to a node over an [`OtaTransport`]; [`Ota`] is the runtime
//! the control plane drives — it holds the store, the registered per-node
//! transports, the job table and the optional [`MetricSink`] the outcome rows
//! are written to.
//!
//! Nothing here touches the network directly: the transport seam makes the
//! production UDP path and the deterministic fake the same code.

mod manager;
mod store;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use aquila_metrics::{
    Authority, CommitSha, Grade, HostFingerprint, Measurement, MeasurementValue, MetricName,
    MetricSink, Timestamp, ToolVersions, Unit,
};
use aquila_types::CONTRACT_VERSION;
use serde_json::{json, Value};

pub use manager::{OtaConfig, OtaError, OtaManager, OtaStats, OtaTransport};
pub use store::{
    FirmwareCompat, FirmwareManifest, FirmwareStore, ImportRequest, StoreError, StoredImage,
    DEFAULT_FIRMWARE_DIR,
};

/// The lifecycle state of one firmware job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareJobState {
    /// No transfer has started.
    Idle,
    /// A transfer is in progress.
    Running,
    /// The transfer completed and the node activated the image.
    Succeeded,
    /// The transfer or verification failed.
    Failed,
    /// The transfer was aborted.
    Aborted,
}

impl FirmwareJobState {
    /// The stable lower-case label used in the JSON representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Aborted => "aborted",
        }
    }
}

/// One firmware OTA job (`firmware-ota.md` §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareJob {
    /// The job identity returned by `POST .../firmware/install`.
    pub job_id: String,
    /// The node the job targets, as the canonical lower-case MAC.
    pub node: String,
    /// The store identity of the image.
    pub image_id: String,
    /// The job lifecycle state.
    pub state: FirmwareJobState,
    /// The whole image size in bytes.
    pub total_bytes: u64,
    /// The cumulative bytes the node acknowledged.
    pub received_bytes: u64,
    /// How many window retransmissions occurred.
    pub retransmissions: u32,
    /// A human-readable result, present once the job is no longer running.
    pub result: Option<String>,
}

impl FirmwareJob {
    /// The job as the control plane serves it (`core-api.md` §6, §7).
    #[must_use]
    pub fn to_json(&self) -> Value {
        let progress = if self.total_bytes == 0 {
            0.0
        } else {
            self.received_bytes as f64 / self.total_bytes as f64
        };
        json!({
            "jobId": self.job_id,
            "node": self.node,
            "imageId": self.image_id,
            "state": self.state.as_str(),
            "progress": {
                "receivedBytes": self.received_bytes,
                "totalBytes": self.total_bytes,
                "fraction": progress,
            },
            "retransmissions": self.retransmissions,
            "result": self.result,
        })
    }
}

/// Why an install request could not be started or completed.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    /// No store and no transport are configured: OTA genuinely cannot run.
    #[error("firmware OTA is not configured in this build")]
    Unavailable,
    /// The requested image is not in the store.
    #[error("firmware image {0} is not in the store")]
    ImageNotFound(String),
    /// No transport is registered for the node, so the command cannot reach it.
    #[error("node {0} has no control transport registered")]
    NodeOffline(String),
    /// The store rejected the image.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The transfer failed.
    #[error(transparent)]
    Transfer(#[from] OtaError),
}

/// The control plane's OTA runtime: the store, the per-node transports, the job
/// table and the optional metric sink.
pub struct Ota {
    store: Mutex<Option<Arc<FirmwareStore>>>,
    manager: OtaManager,
    sink: Mutex<Option<Box<dyn MetricSink>>>,
    jobs: Mutex<BTreeMap<String, FirmwareJob>>,
    transports: Mutex<BTreeMap<String, Box<dyn OtaTransport>>>,
    next_job: AtomicU64,
}

impl std::fmt::Debug for Ota {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ota")
            .field("store", &self.has_store())
            .field("manager", &self.manager)
            .field("jobs", &lock(&self.jobs).len())
            .field("transports", &lock(&self.transports).len())
            .finish()
    }
}

impl Default for Ota {
    fn default() -> Self {
        Self::unconfigured()
    }
}

impl Ota {
    /// An OTA runtime with no store and no transports: every install fails
    /// closed with [`InstallError::Unavailable`].
    #[must_use]
    pub fn unconfigured() -> Self {
        Self {
            store: Mutex::new(None),
            manager: OtaManager::new(OtaConfig::default()),
            sink: Mutex::new(None),
            jobs: Mutex::new(BTreeMap::new()),
            transports: Mutex::new(BTreeMap::new()),
            next_job: AtomicU64::new(1),
        }
    }

    /// An OTA runtime over a signed image store.
    #[must_use]
    pub fn with_store(store: FirmwareStore) -> Self {
        let ota = Self::unconfigured();
        ota.configure_store(store);
        ota
    }

    /// Install (or replace) the signed image store.
    pub fn configure_store(&self, store: FirmwareStore) {
        *lock(&self.store) = Some(Arc::new(store));
    }

    /// Set the metric sink that outcome rows are written to.
    pub fn set_metric_sink(&self, sink: Box<dyn MetricSink>) {
        *lock(&self.sink) = Some(sink);
    }

    /// Whether a signed image store is configured.
    #[must_use]
    pub fn has_store(&self) -> bool {
        lock(&self.store).is_some()
    }

    /// The configured store, if any.
    #[must_use]
    pub fn store(&self) -> Option<Arc<FirmwareStore>> {
        lock(&self.store).clone()
    }

    /// Register the control transport for a node, keyed by lower-case MAC or
    /// dense id string (the same keys the topology projections use).
    pub fn register_transport(&self, node: impl Into<String>, transport: Box<dyn OtaTransport>) {
        lock(&self.transports).insert(node.into().to_ascii_lowercase(), transport);
    }

    /// Whether a transport is registered for `node`.
    #[must_use]
    pub fn has_transport(&self, node: &str) -> bool {
        lock(&self.transports).contains_key(&node.to_ascii_lowercase())
    }

    /// The stored image manifest list, shaped as `{ "items": [...] }`.
    #[must_use]
    pub fn store_json(&self) -> Value {
        let items: Vec<Value> = match self.store() {
            Some(store) => store
                .list()
                .into_iter()
                .map(|manifest| {
                    json!({
                        "id": manifest.id(),
                        "version": manifest.version,
                        "size": manifest.size,
                        "sha256": manifest.sha256,
                        "target": manifest.target,
                        "compatibility": {
                            "minCore": manifest.compatibility.min_core,
                            "maxCore": manifest.compatibility.max_core,
                        },
                        "notes": manifest.notes,
                    })
                })
                .collect(),
            None => Vec::new(),
        };
        json!({ "items": items })
    }

    /// The highest stored version, for a node's `latest_version`.
    #[must_use]
    pub fn latest_version(&self) -> Option<String> {
        self.store()?
            .list()
            .into_iter()
            .map(|manifest| manifest.version)
            .max()
    }

    /// The job for a node, if one has been started.
    #[must_use]
    pub fn job(&self, node: &str) -> Option<FirmwareJob> {
        lock(&self.jobs).get(&node.to_ascii_lowercase()).cloned()
    }

    /// Abort a node's job. Returns `true` if a job was present.
    ///
    /// Transfers in this build are driven synchronously, so this marks the job
    /// aborted; a future asynchronous driver would also signal the transfer.
    pub fn abort(&self, node: &str) -> bool {
        let mut jobs = lock(&self.jobs);
        match jobs.get_mut(&node.to_ascii_lowercase()) {
            Some(job) => {
                job.state = FirmwareJobState::Aborted;
                job.result = Some("aborted by operator".to_string());
                true
            }
            None => false,
        }
    }

    /// Begin an OTA job: verify the image is offered, then drive the transfer
    /// synchronously and record the outcome.
    ///
    /// # Errors
    ///
    /// Returns a typed [`InstallError`] for each failure mode.
    pub fn install(
        &self,
        node: &str,
        node_target: &str,
        image_id: &str,
        force: bool,
    ) -> Result<FirmwareJob, InstallError> {
        let store = self.store().ok_or(InstallError::Unavailable)?;
        let manifest = store
            .manifest(image_id)
            .ok_or_else(|| InstallError::ImageNotFound(image_id.to_string()))?;
        let stored = store
            .get(image_id)?
            .ok_or_else(|| InstallError::ImageNotFound(image_id.to_string()))?;
        let core_version = FirmwareCompat::encode(CONTRACT_VERSION.major, CONTRACT_VERSION.minor);

        // An incompatible image is refused unless forced.
        if !force && !store.offers(image_id, node_target, core_version) {
            return Err(InstallError::Store(StoreError::Incompatible {
                core: core_version,
                min: manifest.compatibility.min_core,
                max: manifest.compatibility.max_core,
            }));
        }

        let mut transport = lock(&self.transports)
            .remove(&node.to_ascii_lowercase())
            .ok_or_else(|| InstallError::NodeOffline(node.to_string()))?;

        let job_id = format!("fw-{}", self.next_job.fetch_add(1, Ordering::SeqCst));
        let total = stored.image.len() as u64;
        lock(&self.jobs).insert(
            node.to_ascii_lowercase(),
            FirmwareJob {
                job_id: job_id.clone(),
                node: node.to_ascii_lowercase(),
                image_id: image_id.to_string(),
                state: FirmwareJobState::Running,
                total_bytes: total,
                received_bytes: 0,
                retransmissions: 0,
                result: None,
            },
        );

        let outcome = self
            .manager
            .transfer(&stored.image, &manifest, transport.as_mut());

        // Return the transport to the registry so a later retry can resume.
        lock(&self.transports)
            .insert(node.to_ascii_lowercase(), transport);

        let job = match outcome {
            Ok(stats) => {
                self.record_rows(true, &manifest);
                FirmwareJob {
                    job_id,
                    node: node.to_ascii_lowercase(),
                    image_id: image_id.to_string(),
                    state: FirmwareJobState::Succeeded,
                    total_bytes: total,
                    received_bytes: stats.received_bytes,
                    retransmissions: stats.retransmissions,
                    result: Some(format!("{:?}", stats.outcome).to_ascii_lowercase()),
                }
            }
            Err(error) => {
                self.record_rows(false, &manifest);
                FirmwareJob {
                    job_id,
                    node: node.to_ascii_lowercase(),
                    image_id: image_id.to_string(),
                    state: FirmwareJobState::Failed,
                    total_bytes: total,
                    received_bytes: 0,
                    retransmissions: 0,
                    result: Some(error.to_string()),
                }
            }
        };
        lock(&self.jobs)
            .insert(node.to_ascii_lowercase(), job.clone());
        Ok(job)
    }

    /// Record the two ledger rows for a completed job (`firmware-ota.md` §9).
    fn record_rows(&self, success: bool, manifest: &FirmwareManifest) {
        let mut sink = lock(&self.sink);
        let sink = match sink.as_mut() {
            Some(sink) => sink,
            None => return,
        };
        let _ = sink.record(&ota_result_row(success));
        let _ = sink.record(&installed_version_row(manifest));
    }
}

/// The `firmware.ota.result` ledger row.
fn ota_result_row(success: bool) -> Measurement {
    Measurement {
        name: MetricName::new("firmware.ota.result".to_string()).expect("valid metric name"),
        version: 1,
        value: MeasurementValue::Flag(success),
        unit: Unit::new("flag").expect("valid unit"),
        samples: 1,
        seed: None,
        commit: CommitSha::new("0".repeat(40)).expect("valid commit"),
        dirty: false,
        host: HostFingerprint::new("aquila-api").expect("valid host"),
        tools: ToolVersions::new(),
        authority: Authority::Measured,
        grade: Grade::Deterministic,
        timestamp: Timestamp::from_unix_ms(0),
    }
}

/// The `firmware.version.installed` ledger row.
fn installed_version_row(manifest: &FirmwareManifest) -> Measurement {
    Measurement {
        name: MetricName::new("firmware.version.installed".to_string())
            .expect("valid metric name"),
        version: 1,
        value: MeasurementValue::Digest(manifest.sha256.clone()),
        unit: Unit::new("sha256").expect("valid unit"),
        samples: 1,
        seed: None,
        commit: CommitSha::new("0".repeat(40)).expect("valid commit"),
        dirty: false,
        host: HostFingerprint::new("aquila-api").expect("valid host"),
        tools: ToolVersions::new(),
        authority: Authority::Measured,
        grade: Grade::Deterministic,
        timestamp: Timestamp::from_unix_ms(0),
    }
}

/// Lock a mutex, recovering from a poisoned lock rather than panicking.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
