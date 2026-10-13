//! The [`MetricSink`] seam and its v1 adapters.
//!
//! The seam is write-only and deliberately small (`measurement.md` §3):
//! reading and querying live in the ledger store, never behind the sink.
//! [`JsonLedgerSink`] is the production adapter; [`InMemoryMetricSink`] is the
//! in-tree fake; [`TeeSink`] fans one record out to several sinks and fails
//! closed if any required sink refuses it.
//!
//! Every sink is idempotent on a record's derived identity: recording the same
//! measurement twice yields one row, because a re-recording differs only in a
//! `timestamp` that never participates in identity.

use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::MetricsError;
use crate::measurement::{Measurement, MeasurementId};

/// The one write path out of the measurement module.
pub trait MetricSink: Send {
    /// Record one measurement.
    fn record(&mut self, measurement: &Measurement) -> Result<(), MetricsError>;

    /// Flush buffered adapters. The default is a no-op.
    fn flush(&mut self) -> Result<(), MetricsError> {
        Ok(())
    }
}

/// The append-only JSON Lines ledger, the source of truth for
/// [`diff`](crate::diff).
///
/// Each record is written as one canonical line: no insignificant whitespace
/// and sorted tool keys. Opening an existing ledger reads it back so identity
/// dedup survives a restart; the file is never rewritten.
pub struct JsonLedgerSink {
    path: PathBuf,
    file: std::fs::File,
    seen: BTreeSet<MeasurementId>,
}

impl std::fmt::Debug for JsonLedgerSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsonLedgerSink")
            .field("path", &self.path)
            .field("records", &self.seen.len())
            .finish()
    }
}

impl JsonLedgerSink {
    /// Open (or create) a JSONL ledger at `path`, reading any existing records
    /// so their identities are not written twice.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, MetricsError> {
        let path = path.into();
        let mut seen = BTreeSet::new();
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            for line in contents.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                let existing = Measurement::from_canonical_json(line)?;
                seen.insert(existing.id());
            }
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self { path, file, seen })
    }

    /// The ledger path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether this ledger already holds the record with `id`.
    #[must_use]
    pub fn contains(&self, id: &MeasurementId) -> bool {
        self.seen.contains(id)
    }

    /// The number of distinct record identities written.
    #[must_use]
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    /// Whether the ledger holds no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

impl MetricSink for JsonLedgerSink {
    fn record(&mut self, measurement: &Measurement) -> Result<(), MetricsError> {
        measurement.validate()?;
        let id = measurement.id();
        if self.seen.contains(&id) {
            return Ok(());
        }
        let line = measurement.to_canonical_json()?;
        writeln!(self.file, "{line}")?;
        self.seen.insert(id);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), MetricsError> {
        self.file.flush()?;
        Ok(())
    }
}

/// An in-memory fake that collects records for TDD assertions.
#[derive(Debug, Default)]
pub struct InMemoryMetricSink {
    records: Vec<Measurement>,
    seen: BTreeSet<MeasurementId>,
}

impl InMemoryMetricSink {
    /// Construct an empty sink.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The records collected so far, in insertion order.
    #[must_use]
    pub fn records(&self) -> &[Measurement] {
        &self.records
    }

    /// Consume the sink, returning the collected records.
    #[must_use]
    pub fn into_records(self) -> Vec<Measurement> {
        self.records
    }

    /// Whether the sink already holds the record with `id`.
    #[must_use]
    pub fn contains(&self, id: &MeasurementId) -> bool {
        self.seen.contains(id)
    }

    /// The number of distinct records collected.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether no records have been collected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl MetricSink for InMemoryMetricSink {
    fn record(&mut self, measurement: &Measurement) -> Result<(), MetricsError> {
        measurement.validate()?;
        let id = measurement.id();
        if self.seen.insert(id) {
            self.records.push(measurement.clone());
        }
        Ok(())
    }
}

/// Fan one record out to several sinks, failing closed if any required sink
/// refuses it.
///
/// Every sink is attempted even when an earlier one fails; the first failure is
/// returned once all sinks have been tried, so one broken sink cannot silently
/// hide the others.
#[derive(Default)]
pub struct TeeSink {
    sinks: Vec<Box<dyn MetricSink>>,
}

impl std::fmt::Debug for TeeSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TeeSink")
            .field("sinks", &self.sinks.len())
            .finish()
    }
}

impl TeeSink {
    /// Construct a tee over the given sinks.
    #[must_use]
    pub fn new(sinks: Vec<Box<dyn MetricSink>>) -> Self {
        Self { sinks }
    }

    /// Add a sink to the fan-out.
    pub fn push(&mut self, sink: Box<dyn MetricSink>) {
        self.sinks.push(sink);
    }

    /// The number of sinks in the fan-out.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sinks.len()
    }

    /// Whether the fan-out has no sinks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sinks.is_empty()
    }
}

impl MetricSink for TeeSink {
    fn record(&mut self, measurement: &Measurement) -> Result<(), MetricsError> {
        let mut first_error = None;
        for sink in &mut self.sinks {
            if let Err(error) = sink.record(measurement) {
                first_error.get_or_insert(error);
            }
        }
        match first_error {
            Some(error) => Err(MetricsError::RequiredSink(error.to_string())),
            None => Ok(()),
        }
    }

    fn flush(&mut self) -> Result<(), MetricsError> {
        let mut first_error = None;
        for sink in &mut self.sinks {
            if let Err(error) = sink.flush() {
                first_error.get_or_insert(error);
            }
        }
        match first_error {
            Some(error) => Err(MetricsError::RequiredSink(error.to_string())),
            None => Ok(()),
        }
    }
}
