//! `HaSink`: the `Sink` adapter that folds events into the HA entity model.
//!
//! The sink is a thin adapter over [`EntityModel`]: it holds the model behind a
//! shared handle so the composition root (or `aquila-api`) can read the
//! projected state after every step, while remaining a real adapter behind the
//! `aquila-sensing` `Sink` seam.

use std::sync::{Arc, Mutex};

use aquila_sensing::{Sink, SinkError};
use aquila_types::Event;

use crate::entity::EntityModel;

/// A `Sink` that projects each event into an HA-shaped [`EntityModel`].
#[derive(Debug, Clone)]
pub struct HaSink {
    model: Arc<Mutex<EntityModel>>,
}

impl HaSink {
    /// Wrap an entity model in a sink.
    #[must_use]
    pub fn new(model: EntityModel) -> Self {
        Self {
            model: Arc::new(Mutex::new(model)),
        }
    }

    /// A shared handle to the entity model, so a caller can read the projected
    /// state after every step.
    #[must_use]
    pub fn handle(&self) -> Arc<Mutex<EntityModel>> {
        Arc::clone(&self.model)
    }

    /// A snapshot clone of the current entity model.
    #[must_use]
    pub fn model(&self) -> EntityModel {
        lock(&self.model).clone()
    }
}

impl Sink for HaSink {
    fn emit(&mut self, event: &Event) -> Result<(), SinkError> {
        lock(&self.model).apply(event);
        Ok(())
    }
}

/// Lock a mutex, recovering from a poisoned lock rather than panicking.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
