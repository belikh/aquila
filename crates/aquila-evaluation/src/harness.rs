//! The evaluation harness: drive the core, pair estimates with reference
//! records, and score (`evaluation.md` §3–§4).
//!
//! [`Harness::run`] is deliberately small: it takes the core's emitted events
//! and a [`GroundTruth`] adapter, pairs each event with the reference record for
//! the same window and scope, and returns an [`EvaluationReport`]. It never
//! triggers measurement on a live path; it is an offline scorer.

use aquila_types::{CoreTime, Event, EventPayload, SessionId, TimeWindow};

use crate::error::EvaluationError;
use crate::ground_truth::{Coverage, EvalScope, GroundTruth, ReferenceProvenance};

/// The offline scorer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Harness;

/// The result of scoring one run (`evaluation.md` §4).
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluationReport {
    /// The number of estimate/reference pairs scored.
    pub paired_samples: usize,
    /// Presence true positives.
    pub true_positives: u64,
    /// Presence false positives.
    pub false_positives: u64,
    /// Presence false negatives.
    pub false_negatives: u64,
    /// Presence true negatives.
    pub true_negatives: u64,
    /// Presence F1, when at least one positive estimate or reference exists.
    pub presence_f1: Option<f64>,
    /// Count mean absolute error, over paired count samples.
    pub count_mae: Option<f64>,
    /// The coverage of the scored set.
    pub coverage: Coverage,
    /// The reference adapter's provenance.
    pub provenance: ReferenceProvenance,
}

impl Harness {
    /// Construct a harness.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Score `events` against `truth` over `scope`.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationError::NoSamples`] when no event pairs with a
    /// reference record: a metric with zero paired samples is **not measured**
    /// (`evaluation.md` §3.1), never a fabricated zero.
    pub fn run(
        &self,
        truth: &dyn GroundTruth,
        session: &SessionId,
        events: &[Event],
        scope: &EvalScope,
    ) -> Result<EvaluationReport, EvaluationError> {
        let mut paired = 0usize;
        let mut true_positives = 0u64;
        let mut false_positives = 0u64;
        let mut false_negatives = 0u64;
        let mut true_negatives = 0u64;
        let mut count_error = 0.0f64;
        let mut count_pairs = 0u64;

        for event in events {
            let t_ns = event.t_unix_ms.max(0) as u64 * 1_000_000;
            let window = TimeWindow::new(CoreTime(t_ns), CoreTime(t_ns + 1_000_000))
                .map_err(|e| EvaluationError::Alignment(e.to_string()))?;
            let Some(reference) = truth.label(session, window) else {
                continue;
            };
            match (&event.payload, &reference.payload) {
                (
                    EventPayload::Presence {
                        occupied: estimate, ..
                    },
                    EventPayload::Presence {
                        occupied: reference, ..
                    },
                ) => {
                    paired += 1;
                    match (*estimate, *reference) {
                        (true, true) => true_positives += 1,
                        (true, false) => false_positives += 1,
                        (false, true) => false_negatives += 1,
                        (false, false) => true_negatives += 1,
                    }
                }
                (
                    EventPayload::Count { people: estimate },
                    EventPayload::Count { people: reference },
                ) => {
                    paired += 1;
                    count_error += (f64::from(*estimate) - f64::from(*reference)).abs();
                    count_pairs += 1;
                }
                _ => {}
            }
        }

        if paired == 0 {
            return Err(EvaluationError::NoSamples(format!(
                "no event paired with a {:?} reference record for session {}",
                truth.kind(),
                session.as_str()
            )));
        }

        Ok(EvaluationReport {
            paired_samples: paired,
            true_positives,
            false_positives,
            false_negatives,
            true_negatives,
            presence_f1: presence_f1(true_positives, false_positives, false_negatives),
            count_mae: (count_pairs > 0).then(|| count_error / count_pairs as f64),
            coverage: truth.coverage(scope),
            provenance: truth.provenance(),
        })
    }
}

/// F1 over the positive class, or `None` when there is nothing to score.
#[must_use]
pub fn presence_f1(tp: u64, fp: u64, f_n: u64) -> Option<f64> {
    if tp == 0 && fp == 0 && f_n == 0 {
        return None;
    }
    let precision = (tp + fp > 0).then(|| tp as f64 / (tp + fp) as f64);
    let recall = (tp + f_n > 0).then(|| tp as f64 / (tp + f_n) as f64);
    match (precision, recall) {
        (Some(p), Some(r)) if p + r > 0.0 => Some(2.0 * p * r / (p + r)),
        (Some(_), Some(_)) => Some(0.0),
        _ => None,
    }
}
