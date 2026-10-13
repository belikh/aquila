//! The [`Metric`] seam: a metric's identity and its computation.
//!
//! A metric owns its own inputs, so [`Metric::compute`] is pure — no clock, no
//! I/O, no hidden state. Everything that can move a number (seed, host,
//! toolchain) is recorded on the [`Measurement`] it returns, not read from the
//! environment here. The companion helper [`record_metric`] is the one path
//! from a metric to a [`MetricSink`](crate::MetricSink).

use crate::error::MetricsError;
use crate::measurement::{Measurement, MetricName};
use crate::sink::MetricSink;

/// A metric definition: a stable identity and a pure computation.
pub trait Metric {
    /// The stable metric name. This is the ledger key's first component and
    /// must equal the `name` of every [`Measurement`] the metric computes.
    fn id(&self) -> MetricName;

    /// Compute the measurement. Pure: given the metric's captured inputs it
    /// returns the same record every time.
    fn compute(&self) -> Measurement;
}

/// Compute `metric` and write it to `sink`, returning the record that was
/// recorded.
///
/// The sink decides idempotence: an append-only sink with this record's
/// identity already present records nothing, so calling `record_metric` twice
/// with identical inputs yields one row.
pub fn record_metric<S>(sink: &mut S, metric: &dyn Metric) -> Result<Measurement, MetricsError>
where
    S: MetricSink + ?Sized,
{
    let measurement = metric.compute();
    measurement.validate()?;
    sink.record(&measurement)?;
    Ok(measurement)
}
