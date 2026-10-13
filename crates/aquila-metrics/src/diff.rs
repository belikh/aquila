//! The baseline-to-head comparison (`measurement.md` §5–6).
//!
//! [`diff`] reads every metric present in either side and reports one
//! [`Delta`] per metric, in canonical name order. It records and surfaces; it
//! gates nothing. A dirty head, a host mismatch on a host-sensitive metric, or
//! a change of `version`, `unit` or `grade` is a **downgrade**: labelled on the
//! delta, never hidden.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::measurement::{Grade, Measurement, MeasurementValue, MetricName};

/// The default statistical tolerance band, ±5 %.
pub const DEFAULT_TOLERANCE_PCT: f64 = 5.0;

/// The verdict for one metric's comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Present only in the head set.
    Added,
    /// Present only in the baseline set.
    Removed,
    /// Byte-identical or within tolerance with no direction change.
    Unchanged,
    /// A change in the metric's better direction.
    Improved,
    /// A change in the metric's worse direction.
    Regressed,
    /// A deterministic change with no known better/worse direction.
    Changed,
    /// A statistical change inside the tolerance band.
    WithinTolerance,
    /// A statistical change outside the tolerance band.
    OutsideTolerance,
    /// A gated metric at or above/below its bar, in the correct direction.
    GatePass,
    /// A gated metric on the wrong side of its bar.
    GateFail,
    /// A host-sensitive metric compared across differing host fingerprints;
    /// informational only.
    HostMismatch,
    /// The two sides disagree on `version`, `unit` or `grade`; not comparable.
    Uncomparable,
}

/// Why a comparison was downgraded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Downgrade {
    /// The head record was taken on a dirty working tree.
    DirtyHead,
    /// The two sides have different host fingerprints and the metric is
    /// host-sensitive.
    HostMismatch,
    /// The metric definition version differs between the sides.
    VersionChanged,
    /// The unit differs between the sides.
    UnitChanged,
    /// The grade differs between the sides.
    GradeChanged,
}

/// The direction in which a change is an improvement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// A smaller value is better, for example latency or instruction count.
    LowerIsBetter,
    /// A larger value is better, for example coverage or F1.
    HigherIsBetter,
    /// No declared direction; a numeric change is reported neutrally.
    Unknown,
}

/// A declared bar for a gated metric.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    /// The threshold the head value is compared against.
    pub threshold: f64,
    /// The direction in which the threshold reads.
    pub direction: Direction,
}

/// One metric's comparison between the baseline and the head.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delta {
    /// The metric name.
    pub name: MetricName,
    /// The definition version compared (the head's when present).
    pub version: u32,
    /// The grade compared (the head's when present).
    pub grade: Grade,
    /// The verdict.
    pub verdict: Verdict,
    /// The baseline value, when the metric is present there.
    pub baseline: Option<MeasurementValue>,
    /// The head value, when the metric is present there.
    pub head: Option<MeasurementValue>,
    /// The signed relative change `(head - baseline) / baseline`, for numeric
    /// values only.
    pub change: Option<f64>,
    /// The declared bar, for a gated metric.
    pub bar: Option<f64>,
    /// The raw margin `head - bar`, for a gated metric.
    pub margin: Option<f64>,
    /// The downgrades applied to this comparison, in declaration order.
    pub downgrades: Vec<Downgrade>,
}

impl Delta {
    /// Whether any honesty guard downgraded this comparison.
    #[must_use]
    pub fn is_downgraded(&self) -> bool {
        !self.downgrades.is_empty()
    }
}

/// The tunable policy for a comparison.
///
/// The defaults encode the v1 catalogue: a ±5 % statistical band, the
/// better-direction of every named metric, and the three gated accuracy bars.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffOptions {
    /// The statistical tolerance band as a percentage.
    pub tolerance_pct: f64,
    /// Ordered name prefixes and their better-direction.
    pub directions: Vec<(String, Direction)>,
    /// Ordered name prefixes and their gated bar.
    pub bars: Vec<(String, Bar)>,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            tolerance_pct: DEFAULT_TOLERANCE_PCT,
            directions: vec![
                ("micro.".to_string(), Direction::LowerIsBetter),
                ("system.latency.".to_string(), Direction::LowerIsBetter),
                ("system.rss.".to_string(), Direction::LowerIsBetter),
                (
                    "accuracy.breathing.mae_brpm".to_string(),
                    Direction::LowerIsBetter,
                ),
                (
                    "accuracy.heart_rate.mae_bpm".to_string(),
                    Direction::LowerIsBetter,
                ),
                (
                    "correctness.coverage.".to_string(),
                    Direction::HigherIsBetter,
                ),
                (
                    "correctness.mutation.score".to_string(),
                    Direction::HigherIsBetter,
                ),
                (
                    "accuracy.torso_pck_at_20".to_string(),
                    Direction::HigherIsBetter,
                ),
                (
                    "accuracy.presence_f1".to_string(),
                    Direction::HigherIsBetter,
                ),
                (
                    "system.frames.per_second".to_string(),
                    Direction::HigherIsBetter,
                ),
                (
                    "structure.docs.ratio".to_string(),
                    Direction::HigherIsBetter,
                ),
            ],
            bars: vec![
                (
                    "accuracy.torso_pck_at_20".to_string(),
                    Bar {
                        threshold: 65.0,
                        direction: Direction::HigherIsBetter,
                    },
                ),
                (
                    "accuracy.presence_f1".to_string(),
                    Bar {
                        threshold: 0.9,
                        direction: Direction::HigherIsBetter,
                    },
                ),
                (
                    "accuracy.breathing.mae_brpm".to_string(),
                    Bar {
                        threshold: 2.0,
                        direction: Direction::LowerIsBetter,
                    },
                ),
            ],
        }
    }
}

impl DiffOptions {
    /// The declared direction for a metric, or [`Direction::Unknown`].
    #[must_use]
    pub fn direction_of(&self, name: &str) -> Direction {
        self.directions
            .iter()
            .find(|(prefix, _)| name.starts_with(prefix.as_str()))
            .map_or(Direction::Unknown, |(_, direction)| *direction)
    }

    /// The declared bar for a metric, or `None`.
    #[must_use]
    pub fn bar_for(&self, name: &str) -> Option<Bar> {
        self.bars
            .iter()
            .find(|(prefix, _)| name.starts_with(prefix.as_str()))
            .map(|(_, bar)| *bar)
    }
}

/// Compare a baseline measurement set with a head set, returning one
/// [`Delta`] per metric in canonical name order.
///
/// Uses [`DiffOptions::default`]; see [`diff_with`] to override the tolerance
/// band, directions or bars.
#[must_use]
pub fn diff(baseline: &[Measurement], head: &[Measurement]) -> Vec<Delta> {
    diff_with(baseline, head, &DiffOptions::default())
}

/// Compare two measurement sets under an explicit policy.
#[must_use]
pub fn diff_with(
    baseline: &[Measurement],
    head: &[Measurement],
    options: &DiffOptions,
) -> Vec<Delta> {
    let baseline = index_by_name(baseline);
    let head = index_by_name(head);

    let mut names: BTreeSet<&MetricName> = BTreeSet::new();
    names.extend(baseline.keys());
    names.extend(head.keys());

    names
        .into_iter()
        .map(|name| {
            compare(
                name,
                baseline.get(name).copied(),
                head.get(name).copied(),
                options,
            )
        })
        .collect()
}

/// Index a measurement set by metric name. If a name appears more than once the
/// last record in input order wins, deterministically.
fn index_by_name(measurements: &[Measurement]) -> BTreeMap<&MetricName, &Measurement> {
    let mut index = BTreeMap::new();
    for measurement in measurements {
        index.insert(&measurement.name, measurement);
    }
    index
}

/// Whether a metric is host-sensitive: micro- and system-performance carry a
/// host fingerprint and are not comparable across hosts.
fn is_host_sensitive(name: &str) -> bool {
    name.starts_with("micro.") || name.starts_with("system.")
}

fn relative_change(base: f64, head: f64) -> Option<f64> {
    if base == 0.0 {
        None
    } else {
        Some((head - base) / base)
    }
}

fn improved_or_regressed(direction: Direction, base: f64, head: f64) -> Verdict {
    match direction {
        Direction::LowerIsBetter if head < base => Verdict::Improved,
        Direction::HigherIsBetter if head > base => Verdict::Improved,
        Direction::Unknown => Verdict::Changed,
        Direction::LowerIsBetter | Direction::HigherIsBetter => Verdict::Regressed,
    }
}

fn compare(
    name: &MetricName,
    baseline: Option<&Measurement>,
    head: Option<&Measurement>,
    options: &DiffOptions,
) -> Delta {
    let (base, head) = match (baseline, head) {
        (None, Some(head)) => {
            return Delta {
                name: name.clone(),
                version: head.version,
                grade: head.grade,
                verdict: Verdict::Added,
                baseline: None,
                head: Some(head.value.clone()),
                change: None,
                bar: None,
                margin: None,
                downgrades: dirty_flag(head),
            };
        }
        (Some(base), None) => {
            return Delta {
                name: name.clone(),
                version: base.version,
                grade: base.grade,
                verdict: Verdict::Removed,
                baseline: Some(base.value.clone()),
                head: None,
                change: None,
                bar: None,
                margin: None,
                downgrades: Vec::new(),
            };
        }
        (Some(base), Some(head)) => (base, head),
        (None, None) => unreachable!("a name is only visited when present on one side"),
    };

    let mut downgrades = dirty_flag(head);
    let mut uncomparable = false;
    if base.version != head.version {
        downgrades.push(Downgrade::VersionChanged);
        uncomparable = true;
    }
    if base.unit != head.unit {
        downgrades.push(Downgrade::UnitChanged);
        uncomparable = true;
    }
    if base.grade != head.grade {
        downgrades.push(Downgrade::GradeChanged);
        uncomparable = true;
    }

    let mut delta = Delta {
        name: name.clone(),
        version: head.version,
        grade: head.grade,
        verdict: Verdict::Unchanged,
        baseline: Some(base.value.clone()),
        head: Some(head.value.clone()),
        change: None,
        bar: None,
        margin: None,
        downgrades,
    };

    if uncomparable {
        delta.verdict = Verdict::Uncomparable;
        return delta;
    }

    if base.host != head.host && is_host_sensitive(name.as_str()) {
        delta.downgrades.push(Downgrade::HostMismatch);
        delta.verdict = Verdict::HostMismatch;
        return delta;
    }

    match head.grade {
        Grade::Deterministic => {
            if base.value == head.value {
                delta.verdict = Verdict::Unchanged;
            } else if let (Some(base_value), Some(head_value)) =
                (base.value.as_f64(), head.value.as_f64())
            {
                delta.change = relative_change(base_value, head_value);
                delta.verdict = improved_or_regressed(
                    options.direction_of(name.as_str()),
                    base_value,
                    head_value,
                );
            } else {
                delta.verdict = Verdict::Changed;
            }
        }
        Grade::Statistical => {
            if let (Some(base_value), Some(head_value)) = (base.value.as_f64(), head.value.as_f64())
            {
                delta.change = relative_change(base_value, head_value);
                let tolerance = options.tolerance_pct / 100.0;
                let within = delta
                    .change
                    .map_or(base_value == head_value, |change| change.abs() <= tolerance);
                delta.verdict = if within {
                    Verdict::WithinTolerance
                } else {
                    match options.direction_of(name.as_str()) {
                        Direction::Unknown => Verdict::OutsideTolerance,
                        direction => improved_or_regressed(direction, base_value, head_value),
                    }
                };
            } else if base.value == head.value {
                delta.verdict = Verdict::Unchanged;
            } else {
                delta.verdict = Verdict::Changed;
            }
        }
        Grade::Gated => {
            if let Some(bar) = options.bar_for(name.as_str()) {
                if let Some(head_value) = head.value.as_f64() {
                    delta.bar = Some(bar.threshold);
                    delta.margin = Some(head_value - bar.threshold);
                    let passed = match bar.direction {
                        Direction::HigherIsBetter => head_value >= bar.threshold,
                        Direction::LowerIsBetter => head_value <= bar.threshold,
                        Direction::Unknown => false,
                    };
                    delta.change = base
                        .value
                        .as_f64()
                        .and_then(|base_value| relative_change(base_value, head_value));
                    delta.verdict = if passed {
                        Verdict::GatePass
                    } else {
                        Verdict::GateFail
                    };
                }
            } else if base.value == head.value {
                delta.verdict = Verdict::Unchanged;
            } else {
                delta.verdict = Verdict::Changed;
            }
        }
    }

    delta
}

fn dirty_flag(head: &Measurement) -> Vec<Downgrade> {
    if head.dirty {
        vec![Downgrade::DirtyHead]
    } else {
        Vec::new()
    }
}
