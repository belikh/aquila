//! The control-plane handle and the `ChannelSink` `Sink` adapter.
//!
//! [`Handle`] is the shared, cheaply-cloneable state of a running server: the
//! HA-shaped [`EntityModel`] the `/api/v1/state` route serves, and the set of
//! live subscribers. [`ChannelSink`] implements the `aquila-sensing` `Sink`
//! seam by publishing every event into that shared state, so a running server
//! actually receives the core's output.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use aquila_ha::EntityModel;
use aquila_sensing::{Sink, SinkError};
use aquila_types::Event;
use serde_json::{json, Value};

/// The shared server state: the latest projected entity model, a monotonic
/// revision, and the subscriber registry.
struct State {
    model: EntityModel,
    started: Instant,
    revision: u64,
    subscribers: Vec<mpsc::Sender<Event>>,
}

/// A cheaply-cloneable handle to a running control plane.
#[derive(Clone)]
pub struct Handle {
    state: Arc<Mutex<State>>,
}

impl Handle {
    /// Construct a handle over an initial entity model.
    pub(crate) fn new(model: EntityModel) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                model,
                started: Instant::now(),
                revision: 0,
                subscribers: Vec::new(),
            })),
        }
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
        state.subscribers.retain(|sender| match sender.send(event.clone()) {
            Ok(()) => true,
            Err(mpsc::SendError(_)) => false,
        });
    }

    /// Subscribe to the published event stream.
    ///
    /// The returned receiver is fed synchronously by [`publish`](Self::publish);
    /// it exists so a co-located consumer (the dashboard, a test) observes the
    /// same events the WebSocket stream will carry.
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
        let uptime_ms = state.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
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
