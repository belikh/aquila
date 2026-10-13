//! The commit-keyed measurement ledger.
//!
//! Measurement is first-class: record and diff, no CI gating yet. Every metric
//! result is appended to the ledger as one [`LedgerRecord`] per line. Records
//! are append-only; a re-measurement supersedes via `supersedes`. A claim's
//! [`Witness`](crate::Witness) is derived from these records: the ledger is the
//! only source of `Gated`/`Measured`.

use serde::{Deserialize, Serialize};

use crate::error::{require_finite_f64, require_non_empty, ValidationError};
use crate::ids::{MetricId, SessionId};
use crate::session::BuildId;
use crate::version::ContractVersion;

/// The epistemic class of a metric, which governs how a change is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    /// Must reproduce bit-identically for identical inputs and environment.
    Deterministic,
    /// Characterised by a distribution; carries `samples` and a confidence
    /// interval.
    Statistical,
    /// Compared against a v1 bar; carries `threshold` and `passed`.
    Gated,
}

/// A confidence interval. Invariant: `low <= high`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    /// The lower bound.
    pub low: f64,
    /// The upper bound.
    pub high: f64,
}

impl Interval {
    /// Validate the ordering and finiteness of the bounds.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_finite_f64("Interval.low", self.low)?;
        require_finite_f64("Interval.high", self.high)?;
        if self.low <= self.high {
            Ok(())
        } else {
            Err(ValidationError::Range {
                field: "Interval",
                detail: "low must not exceed high".to_string(),
            })
        }
    }
}

/// A dataset split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    /// Training split.
    Train,
    /// Validation split.
    Validation,
    /// Test split.
    Test,
    /// In-domain split.
    InDomain,
    /// Out-of-domain split.
    OutOfDomain,
}

/// A reference to the dataset a measurement was taken on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetRef {
    /// The dataset name.
    pub name: String,
    /// The split used.
    pub split: Split,
}

impl DatasetRef {
    /// Validate that the dataset name is non-empty.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_empty("DatasetRef.name", &self.name)
    }
}

/// A reference to the model that produced a measurement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    /// The model identity.
    pub id: String,
    /// SHA-256 digest of the weights.
    pub sha256: [u8; 32],
    /// The ONNX opset the model targets.
    pub onnx_opset: u32,
}

impl ModelRef {
    /// Validate that the model identity is non-empty.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_empty("ModelRef.id", &self.id)
    }
}

/// A fingerprint of the environment a measurement was taken in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvFingerprint {
    /// The host description.
    pub host: String,
    /// The Rust compiler version.
    pub rustc: String,
    /// The operating system.
    pub os: String,
    /// The thread count.
    pub thread_count: u32,
    /// The RNG description, for example `"splitmix64:seed"`.
    pub rng: String,
}

impl EnvFingerprint {
    /// Validate that the descriptive fields are non-empty.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_empty("EnvFingerprint.host", &self.host)?;
        require_non_empty("EnvFingerprint.rustc", &self.rustc)?;
        require_non_empty("EnvFingerprint.os", &self.os)?;
        require_non_empty("EnvFingerprint.rng", &self.rng)
    }
}

/// One commit-keyed measurement result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerRecord {
    /// Deterministic identity: `hash(commit, session, metric, config, dataset)`.
    pub id: String,
    /// The build the record was taken at.
    pub commit: BuildId,
    /// The contract version.
    pub contract: ContractVersion,
    /// The session the measurement was taken on.
    pub session: SessionId,
    /// The metric identity.
    pub metric: MetricId,
    /// The epistemic class of the metric.
    pub kind: MetricKind,
    /// The observed value.
    pub value: f64,
    /// The unit label, for example `"PCK@20"`, `"F1"` or `"brpm"`.
    pub unit: String,
    /// The number of samples aggregated into the value.
    pub samples: u64,
    /// The confidence interval; required for `Statistical`.
    pub ci: Option<Interval>,
    /// The dataset the measurement was taken on.
    pub dataset: DatasetRef,
    /// The model that produced the measurement, when one applies.
    pub model: Option<ModelRef>,
    /// SHA-256 of the canonical configuration.
    pub config_hash: [u8; 32],
    /// The bar the metric was compared against; required for `Gated`.
    pub threshold: Option<f64>,
    /// The bar's verdict; required for `Gated`.
    pub passed: Option<bool>,
    /// The environment fingerprint.
    pub env: EnvFingerprint,
    /// Unix wall time the record was created, in milliseconds.
    pub created_unix_ms: i64,
    /// The prior record id this record supersedes, for diffing history.
    pub supersedes: Option<String>,
}

impl LedgerRecord {
    /// Validate identity, finiteness and the per-kind obligations.
    ///
    /// * `Statistical` requires a confidence interval and `samples > 0`.
    /// * `Gated` requires a threshold and a pass/fail verdict.
    pub fn validate(&self) -> Result<(), ValidationError> {
        require_non_empty("LedgerRecord.id", &self.id)?;
        self.commit.validate()?;
        self.session.validate()?;
        self.metric.validate()?;
        require_non_empty("LedgerRecord.unit", &self.unit)?;
        require_finite_f64("LedgerRecord.value", self.value)?;
        self.dataset.validate()?;
        if let Some(model) = &self.model {
            model.validate()?;
        }
        self.env.validate()?;
        if let Some(ci) = &self.ci {
            ci.validate()?;
        }
        if let Some(threshold) = self.threshold {
            require_finite_f64("LedgerRecord.threshold", threshold)?;
        }

        match self.kind {
            MetricKind::Deterministic => Ok(()),
            MetricKind::Statistical => {
                if self.ci.is_none() {
                    return Err(ValidationError::Invariant {
                        detail: "MetricKind::Statistical requires a confidence interval"
                            .to_string(),
                    });
                }
                if self.samples == 0 {
                    return Err(ValidationError::Invariant {
                        detail: "MetricKind::Statistical requires samples > 0".to_string(),
                    });
                }
                Ok(())
            }
            MetricKind::Gated => {
                if self.threshold.is_none() {
                    return Err(ValidationError::Invariant {
                        detail: "MetricKind::Gated requires a threshold".to_string(),
                    });
                }
                if self.passed.is_none() {
                    return Err(ValidationError::Invariant {
                        detail: "MetricKind::Gated requires a pass/fail verdict".to_string(),
                    });
                }
                Ok(())
            }
        }
    }
}
