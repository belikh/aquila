//! The [`Measurement`] record and its supporting value types.
//!
//! A [`Measurement`] is one commit-keyed observation (`measurement.md` §2). It
//! is flattened into the ledger; its identity is derived, never stored, so a
//! re-recording with a different `timestamp` is the *same* record. The value is
//! tagged so a digest or a byte count is never lossily coerced through `f64`.

use std::collections::BTreeMap;
use std::fmt;

use aquila_types::{sha256, Clock, MetricId};
use serde::{Deserialize, Serialize};

use crate::error::MetricsError;

/// A stable metric name, for example `accuracy.presence_f1` or
/// `micro.instr.ingest.parse`.
///
/// This is the canonical [`MetricId`] from `aquila-types`: a non-empty,
/// human-authorable [`Slug`](aquila_types::Slug). Dimensions such as
/// percentiles, stages and benches are encoded *in the name* so names sort
/// canonically and no separate tag map is needed.
pub type MetricName = MetricId;

/// Provenance: the strength of the evidence behind a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authority {
    /// Executed on this host, now.
    Measured,
    /// Produced from committed recorded fixtures or replay (sim-first).
    Replayed,
    /// Computed from other measurements, for example a mutation score.
    Derived,
    /// Brought in from an external system, for example an OpenTelemetry feed.
    Imported,
    /// Hand-entered baseline or a research-only ground truth.
    Declared,
}

/// The epistemic class that governs how [`diff`](crate::diff) reads a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    /// Same inputs imply a byte-identical value on a pinned toolchain.
    Deterministic,
    /// Run-to-run variance; compared as a distribution, never asserted exactly.
    Statistical,
    /// Statistical in nature, plus an explicit threshold bar.
    Gated,
}

/// A tagged observation value: never a bare `f64`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementValue {
    /// Continuous quantities: milliseconds, percent, brpm, ratio, score.
    Scalar(f64),
    /// Integral counts: tests, DAG violations, bytes, unsafe blocks.
    Count(u64),
    /// Content hashes: a reference-pipeline SHA-256, an artefact or model digest.
    Digest(String),
    /// Booleans: for example DAG-clean, or unsafe-forbidden holds.
    Flag(bool),
}

impl MeasurementValue {
    /// The value as `f64` when it is numeric ([`Scalar`](Self::Scalar) or
    /// [`Count`](Self::Count)); `None` for a digest or a flag.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Scalar(value) => Some(*value),
            Self::Count(value) => Some(*value as f64),
            Self::Digest(_) | Self::Flag(_) => None,
        }
    }

    /// Validate the value's own invariants: a scalar must be finite and a
    /// digest must be non-empty.
    pub fn validate(&self) -> Result<(), MetricsError> {
        match self {
            Self::Scalar(value) => {
                if value.is_finite() {
                    Ok(())
                } else {
                    Err(MetricsError::Invalid(
                        "MeasurementValue::Scalar must be finite".to_string(),
                    ))
                }
            }
            Self::Count(_) | Self::Flag(_) => Ok(()),
            Self::Digest(digest) => {
                if digest.is_empty() {
                    Err(MetricsError::Invalid(
                        "MeasurementValue::Digest must be non-empty".to_string(),
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// The unit of a [`Measurement`] value.
///
/// A newtype over the stable label so the v1 labels — `"%"`, `"count"`,
/// `"ms"`, `"brpm"`, `"bpm"`, `"bytes"`, `"ratio"`, `"sha256"` and `"score"` —
/// are used consistently without forbidding a new label a later metric needs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Unit(pub String);

impl Unit {
    /// Construct a unit label, rejecting the empty string.
    pub fn new(value: impl Into<String>) -> Result<Self, MetricsError> {
        let value = value.into();
        if value.is_empty() {
            Err(MetricsError::Invalid("unit must be non-empty".to_string()))
        } else {
            Ok(Self(value))
        }
    }

    /// Borrow the unit label.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The exact commit a measurement was taken at: a full 40-character hex SHA.
///
/// A short SHA is not a `CommitSha`; the CLI resolves a commit-ish selector to
/// this full form before a record is written.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommitSha(pub String);

impl CommitSha {
    /// Construct a commit identity from a full 40-character hex SHA.
    pub fn new(value: impl Into<String>) -> Result<Self, MetricsError> {
        let value = value.into();
        if value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
            Ok(Self(value))
        } else {
            Err(MetricsError::Invalid(format!(
                "commit must be a full 40-character hex SHA, got {value:?}"
            )))
        }
    }

    /// Borrow the commit SHA.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A stable digest of host identity: CPU, cores, architecture, OS and runner
/// class.
///
/// Micro- and system-performance are host-dependent, so the fingerprint travels
/// with every record and a cross-host comparison is reported `host-mismatch`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HostFingerprint(pub String);

impl HostFingerprint {
    /// Construct a host fingerprint, rejecting the empty string.
    pub fn new(value: impl Into<String>) -> Result<Self, MetricsError> {
        let value = value.into();
        if value.is_empty() {
            Err(MetricsError::Invalid(
                "host fingerprint must be non-empty".to_string(),
            ))
        } else {
            Ok(Self(value))
        }
    }

    /// Borrow the fingerprint.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Tool name to version for every tool that produced or could influence a
/// value.
///
/// Backed by a [`BTreeMap`] so the keys are always sorted and the canonical
/// serialisation is stable (`measurement.md` §3).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolVersions(pub BTreeMap<String, String>);

impl ToolVersions {
    /// Construct an empty tool-version map.
    #[must_use]
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Insert a tool version, returning any prior version.
    pub fn insert(
        &mut self,
        tool: impl Into<String>,
        version: impl Into<String>,
    ) -> Option<String> {
        self.0.insert(tool.into(), version.into())
    }

    /// The version recorded for a tool, if any.
    #[must_use]
    pub fn get(&self, tool: &str) -> Option<&String> {
        self.0.get(tool)
    }

    /// Iterate the tool versions in sorted key order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.0.iter()
    }

    /// Whether no tool versions are recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The number of recorded tool versions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Validate that no tool name is empty.
    pub fn validate(&self) -> Result<(), MetricsError> {
        if self.0.keys().any(|k| k.is_empty()) {
            Err(MetricsError::Invalid(
                "tool name must be non-empty".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

impl FromIterator<(String, String)> for ToolVersions {
    fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// UTC measurement time as Unix milliseconds.
///
/// Time enters the module only through the [`Clock`] seam
/// ([`Timestamp::now`]); the module never calls `SystemTime::now()`, so tests
/// stay deterministic under a fixed clock. The timestamp is deliberately
/// excluded from a record's identity, so it never participates in comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(pub i64);

impl Timestamp {
    /// Construct a timestamp from Unix milliseconds.
    #[must_use]
    pub const fn from_unix_ms(unix_ms: i64) -> Self {
        Self(unix_ms)
    }

    /// Read "now" from the injected [`Clock`] seam.
    ///
    /// The clock supplies Unix nanoseconds; the ledger stores milliseconds.
    #[must_use]
    pub fn now(clock: &dyn Clock) -> Self {
        Self((clock.now().as_nanos() / 1_000_000) as i64)
    }

    /// The timestamp as Unix milliseconds.
    #[must_use]
    pub const fn as_unix_ms(self) -> i64 {
        self.0
    }
}

/// The derived identity of a measurement: the SHA-256 of the canonically
/// serialised record **excluding** `timestamp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MeasurementId(pub [u8; 32]);

impl MeasurementId {
    /// The raw 32-byte digest.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The digest as lower-case hex.
    #[must_use]
    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(64);
        for byte in self.0 {
            out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
            out.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
        }
        out
    }
}

impl fmt::Display for MeasurementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// A single, commit-keyed observation recorded by the measurement module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    /// Stable metric name, for example `accuracy.presence_f1`.
    pub name: MetricName,
    /// Definition version; bumped whenever the metric's meaning or method
    /// changes.
    pub version: u32,
    /// The observed quantity, tagged so digests and counts are never coerced.
    pub value: MeasurementValue,
    /// Unit of `value`.
    pub unit: Unit,
    /// Observations aggregated into `value` (1 for a single scalar run; N for a
    /// mean).
    pub samples: u32,
    /// Master RNG or fixture seed, or `None` when the metric is seedless.
    pub seed: Option<u64>,
    /// Exact commit the measurement was taken at (full 40-hex SHA).
    pub commit: CommitSha,
    /// True when the working tree had uncommitted changes at record time.
    pub dirty: bool,
    /// Stable digest of host identity.
    pub host: HostFingerprint,
    /// Tool name to version for every tool that produced or influenced value.
    pub tools: ToolVersions,
    /// Provenance of the number: where the evidence came from.
    pub authority: Authority,
    /// Epistemic class that governs diffing.
    pub grade: Grade,
    /// UTC measurement time, taken from the [`Clock`] seam.
    pub timestamp: Timestamp,
}

/// The record's fields that participate in identity, in declaration order,
/// with `timestamp` omitted.
#[derive(Serialize)]
struct MeasurementIdentity<'a> {
    name: &'a MetricName,
    version: u32,
    value: &'a MeasurementValue,
    unit: &'a Unit,
    samples: u32,
    seed: Option<u64>,
    commit: &'a CommitSha,
    dirty: bool,
    host: &'a HostFingerprint,
    tools: &'a ToolVersions,
    authority: Authority,
    grade: Grade,
}

impl Measurement {
    /// The record's derived identity: the SHA-256 of its canonical
    /// serialisation with `timestamp` removed.
    ///
    /// Recording the same measurement twice therefore yields one identity and,
    /// in an append-only sink, one row. The identity cannot fail to compute for
    /// a validated record, because the identity shape serialises infallibly.
    #[must_use]
    pub fn id(&self) -> MeasurementId {
        let identity = MeasurementIdentity {
            name: &self.name,
            version: self.version,
            value: &self.value,
            unit: &self.unit,
            samples: self.samples,
            seed: self.seed,
            commit: &self.commit,
            dirty: self.dirty,
            host: &self.host,
            tools: &self.tools,
            authority: self.authority,
            grade: self.grade,
        };
        let bytes = serde_json::to_vec(&identity)
            .expect("the measurement identity shape serialises infallibly");
        MeasurementId(sha256(&bytes))
    }

    /// The canonical ledger serialisation of the whole record: one line of
    /// JSON, no insignificant whitespace, sorted tool keys.
    pub fn to_canonical_json(&self) -> Result<String, MetricsError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Parse a canonical ledger line back into a measurement.
    pub fn from_canonical_json(line: &str) -> Result<Self, MetricsError> {
        Ok(serde_json::from_str(line)?)
    }

    /// Validate the record's invariants: a non-empty name, a finite scalar or
    /// non-empty digest, a non-empty unit, at least one sample, a non-empty
    /// host and non-empty tool names.
    pub fn validate(&self) -> Result<(), MetricsError> {
        if self.name.as_str().is_empty() {
            return Err(MetricsError::Invalid("name must be non-empty".to_string()));
        }
        self.value.validate()?;
        if self.unit.as_str().is_empty() {
            return Err(MetricsError::Invalid("unit must be non-empty".to_string()));
        }
        if self.samples == 0 {
            return Err(MetricsError::Invalid(
                "samples must be at least 1".to_string(),
            ));
        }
        if self.host.as_str().is_empty() {
            return Err(MetricsError::Invalid(
                "host fingerprint must be non-empty".to_string(),
            ));
        }
        self.tools.validate()
    }
}
