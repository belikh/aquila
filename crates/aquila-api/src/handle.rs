//! The control-plane handle and the `ChannelSink` `Sink` adapter.
//!
//! [`Handle`] is the shared, cheaply-cloneable state of a running server: the
//! HA-shaped [`EntityModel`] the `/api/v1/state` route serves, the
//! topology-derived read-only documents (nodes, schema, firmware), and the set
//! of live subscribers. [`ChannelSink`] implements the `aquila-sensing` `Sink`
//! seam by publishing every event into that shared state, so a running server
//! actually receives the core's output.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use aquila_ha::EntityModel;
use aquila_sensing::{Sink, SinkError};
use aquila_types::Event;
use serde_json::{json, Value};

use crate::ota::Ota;
use crate::projection::Projections;

/// The shared server state: the latest projected entity model, the read-only
/// topology documents, a monotonic revision, and the subscriber registry.
struct State {
    model: EntityModel,
    projections: Projections,
    started: Instant,
    revision: u64,
    subscribers: Vec<mpsc::Sender<Event>>,
}

/// A cheaply-cloneable handle to a running control plane.
#[derive(Clone)]
pub struct Handle {
    state: Arc<Mutex<State>>,
    ota: Arc<Ota>,
}

impl Handle {
    /// Construct a handle over an initial entity model and its topology
    /// projections, with OTA unconfigured.
    pub(crate) fn new(model: EntityModel, projections: Projections) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                model,
                projections,
                started: Instant::now(),
                revision: 0,
                subscribers: Vec::new(),
            })),
            ota: Arc::new(Ota::unconfigured()),
        }
    }

    /// The OTA runtime shared by this handle (`firmware-ota.md` §3).
    ///
    /// The composition root (and the tests) configure the signed image store and
    /// register per-node transports here.
    #[must_use]
    pub fn ota(&self) -> Arc<Ota> {
        Arc::clone(&self.ota)
    }

    /// Publish one event: fold it into the projected model and fan it out to
    /// every live subscriber.
    ///
    /// A subscriber whose receiver has been dropped is pruned; publishing never
    /// blocks and never fails the core (`core-api.md` §10.6).
    pub fn publish(&self, event: &Event) {
        let mut state = lock(&self.state);
        state.model.apply(event);
        state.revision = state.revision.saturating_add(1);
        state
            .subscribers
            .retain(|sender| match sender.send(event.clone()) {
                Ok(()) => true,
                Err(mpsc::SendError(_)) => false,
            });
    }

    /// Subscribe to the published event stream.
    ///
    /// The returned receiver is fed synchronously by [`publish`](Self::publish);
    /// it exists so a co-located consumer (a test, a future dashboard) observes
    /// the same events a transport will carry.
    #[must_use]
    pub fn subscribe(&self) -> mpsc::Receiver<Event> {
        let (sender, receiver) = mpsc::channel();
        lock(&self.state).subscribers.push(sender);
        receiver
    }

    /// The current model revision.
    #[must_use]
    pub fn revision(&self) -> u64 {
        lock(&self.state).revision
    }

    /// The coordinator snapshot for `GET /api/v1/state`.
    pub(crate) fn state_json(&self) -> Value {
        lock(&self.state).model.snapshot()
    }

    /// The health document for `GET /api/v1/health` (`core-api.md` §4.1).
    pub(crate) fn health_json(&self) -> Value {
        let state = lock(&self.state);
        let uptime_ms = state
            .started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        let zones = state.model.zones().len();
        json!({
            "status": "ok",
            "uptimeMs": uptime_ms,
            "api": { "major": 1, "minor": 0 },
            "subsystems": [
                { "name": "sensing", "status": "ok" },
                { "name": "control-plane", "status": "ok",
                  "detail": format!("{} zones projected", zones) },
            ],
        })
    }

    /// The zone tree and metric vocabulary for `GET /api/v1/schema`.
    pub(crate) fn schema_json(&self) -> Value {
        lock(&self.state).projections.schema.clone()
    }

    /// The node list for `GET /api/v1/nodes`.
    pub(crate) fn nodes_json(&self) -> Value {
        lock(&self.state).projections.nodes.clone()
    }

    /// The firmware store manifest for `GET /api/v1/firmware`
    /// (`firmware-ota.md` §7).
    pub(crate) fn firmware_json(&self) -> Value {
        self.ota.store_json()
    }

    /// Per-node firmware state for `GET /api/v1/nodes/{node}/firmware`.
    ///
    /// Looks the node up by lower-case MAC or by dense id string; `None` when
    /// the node is not in the topology. The read-only topology fields are
    /// augmented with the store's latest version and the node's last OTA job.
    pub(crate) fn node_firmware_json(&self, node: &str) -> Option<Value> {
        let mut value = {
            let state = lock(&self.state);
            state
                .projections
                .node_firmware
                .get(&node.to_ascii_lowercase())
                .or_else(|| state.projections.node_firmware.get(node))
                .cloned()?
        };
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "latest_version".to_string(),
                self.ota
                    .latest_version()
                    .map_or(Value::Null, Value::String),
            );
            let job = self.ota.job(node);
            object.insert(
                "pending".to_string(),
                job.as_ref().map_or(Value::Bool(false), |job| {
                    Value::Bool(matches!(
                        job.state,
                        crate::ota::FirmwareJobState::Running
                    ))
                }),
            );
            object.insert(
                "last_result".to_string(),
                job.as_ref().map_or(Value::Null, |job| {
                    json!({
                        "state": job.state.as_str(),
                        "result": job.result,
                        "jobId": job.job_id,
                    })
                }),
            );
        }
        Some(value)
    }

    /// The target-chip string for a topology node (`esp32s3`), if known.
    pub(crate) fn node_target(&self, node: &str) -> Option<String> {
        let state = lock(&self.state);
        state
            .projections
            .node_target
            .get(&node.to_ascii_lowercase())
            .or_else(|| state.projections.node_target.get(node))
            .cloned()
    }

    /// Per-node OTA job progress for `GET /api/v1/nodes/{node}/firmware/job`.
    pub(crate) fn node_firmware_job_json(&self, node: &str) -> Option<Value> {
        // Only an existing topology node owns a job route.
        let state = lock(&self.state);
        let known = state
            .projections
            .node_firmware
            .contains_key(&node.to_ascii_lowercase())
            || state.projections.node_firmware.contains_key(node);
        drop(state);
        if !known {
            return None;
        }
        Some(match self.ota.job(node) {
            Some(job) => job.to_json(),
            None => json!({
                "node": node.to_ascii_lowercase(),
                "state": "idle",
                "progress": Value::Null,
                "result": Value::Null,
            }),
        })
    }

    /// The commit-keyed metric snapshot for `GET /api/v1/metrics/snapshot`
    /// (`core-api.md` §13).
    ///
    /// The ledger is not yet wired into the control plane, so the snapshot is
    /// honestly empty with a live `revision`; it never triggers measurement.
    pub(crate) fn metrics_json(&self) -> Value {
        let revision = lock(&self.state).revision;
        json!({
            "commit": Value::Null,
            "generatedAt": Value::Null,
            "revision": revision,
            "metrics": [],
            "bars": {},
        })
    }
}

/// The `Sink` adapter that publishes the core's events to a running server.
#[derive(Clone)]
pub struct ChannelSink {
    handle: Handle,
}

impl ChannelSink {
    /// Construct a sink bound to a server handle.
    #[must_use]
    pub fn new(handle: Handle) -> Self {
        Self { handle }
    }

    /// The handle the sink publishes to.
    #[must_use]
    pub fn handle(&self) -> &Handle {
        &self.handle
    }
}

impl Sink for ChannelSink {
    fn emit(&mut self, event: &Event) -> Result<(), SinkError> {
        self.handle.publish(event);
        Ok(())
    }
}

/// Lock a mutex, recovering from a poisoned lock rather than panicking.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
