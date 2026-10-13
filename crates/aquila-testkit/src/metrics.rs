//! `RecordingMetricSink`: the in-memory `MetricSink` fake.
//!
//! It collects the [`Measurement`]s the core records so a test can assert at
//! least one row was written, and it is shared so the core can own a handle
//! while the test keeps reading (`testing.md` §3).

use std::sync::{Arc, Mutex};

use aquila_metrics::{Measurement, MetricSink, MetricsError};

/// An in-memory metric sink that records every measurement.
#[derive(Debug, Clone, Default)]
pub struct RecordingMetricSink {
    records: Arc<Mutex<Vec<Measurement>>>,
}

impl RecordingMetricSink {
    /// Construct an empty sink.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A shared handle to the recorded measurements.
    #[must_use]
    pub fn handle(&self) -> Arc<Mutex<Vec<Measurement>>> {
        Arc::clone(&self.records)
    }

    /// A snapshot of the records collected so far.
    #[must_use]
    pub fn records(&self) -> Vec<Measurement> {
        lock(&self.records).clone()
    }

    /// The number of records collected so far.
    #[must_use]
    pub fn len(&self) -> usize {
        lock(&self.records).len()
    }

    /// Whether no records have been collected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        lock(&self.records).is_empty()
    }
}

impl MetricSink for RecordingMetricSink {
    fn record(&mut self, measurement: &Measurement) -> Result<(), MetricsError> {
        measurement.validate()?;
        lock(&self.records).push(measurement.clone());
        Ok(())
    }
}

/// Lock a mutex, recovering from a poisoned lock rather than panicking.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
