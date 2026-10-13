//! The typed failure and configuration surface of the inference seam.
//!
//! [`ModelError`] is the seam's one error type (`inference-models.md` §1.6). It
//! is returned for every *failure to run*; it is never used to express an
//! honest *absence of an estimate*. Absence is expressed through the contract
//! types themselves (a pose with `confidence == 0.0`, a vitals estimate of
//! `None`), because those are legitimate outputs rather than failures.
//!
//! There is no default, zeroed, last-known-good or "best guess" output anywhere
//! in this crate. A failed request yields `Err` and therefore no `Pose`,
//! `VitalsReading` or `Detection` for that span.

use serde::{Deserialize, Serialize};

use aquila_types::ContractVersion;

use crate::model::ModelId;

/// A tensor element type as declared by a model card.
///
/// Only [`DType::F32`] and [`DType::Bool`] appear in the v1 pose and vitals
/// graphs (`inference-models.md` §1.4); the remaining variants exist so a card
/// can describe a future graph honestly rather than coercing its dtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DType {
    /// 32-bit IEEE-754 float.
    F32,
    /// 64-bit IEEE-754 float.
    F64,
    /// Signed 32-bit integer.
    I32,
    /// Signed 64-bit integer.
    I64,
    /// Unsigned 8-bit integer.
    U8,
    /// Boolean.
    Bool,
}

/// A single named tensor dimension.
///
/// The dynamic batch axis is [`Dim::Batch`] (`B` in `inference-models.md`
/// §1.4); every other axis is a fixed extent such as `T`, `S`, `L`, `F`, `K`
/// or `C_MAX + 1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dim {
    /// The dynamic batch axis; any positive extent is admissible.
    Batch,
    /// A fixed extent, in elements.
    Fixed(usize),
}

impl Dim {
    /// Whether this is the dynamic batch axis.
    #[must_use]
    pub const fn is_batch(&self) -> bool {
        matches!(self, Dim::Batch)
    }

    /// The fixed extent, or `None` for the dynamic batch axis.
    #[must_use]
    pub const fn fixed(&self) -> Option<usize> {
        match self {
            Dim::Fixed(n) => Some(*n),
            Dim::Batch => None,
        }
    }
}

/// Device selection for the inference engine.
///
/// Device selection is **construction configuration**, not a seam parameter
/// (`inference-models.md` §1.5). The default and v1 target is CPU; `Cuda` and
/// `OpenVino` are feature-gated and off by default. Because floating point is
/// not bit-identical across execution providers or thread counts, the chosen
/// selection is part of the ledger key so a metric is always tethered to the
/// configuration that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceSelection {
    /// CPU execution with explicit thread-pool sizing. The v1 default.
    Cpu {
        /// Threads used within an operator.
        intra_threads: usize,
        /// Threads used across parallel operators.
        inter_threads: usize,
    },
    /// CUDA execution on a named device. Feature-gated by `cuda`.
    #[cfg(feature = "cuda")]
    Cuda {
        /// The CUDA device ordinal.
        device_id: i32,
    },
    /// OpenVINO execution on a named device. Feature-gated by `openvino`.
    #[cfg(feature = "openvino")]
    OpenVino {
        /// The OpenVINO device name, for example `"CPU"` or `"GPU"`.
        device: String,
    },
}

impl DeviceSelection {
    /// The v1 default: CPU with four intra-op threads and one inter-op thread.
    #[must_use]
    pub const fn cpu_default() -> Self {
        Self::Cpu {
            intra_threads: 4,
            inter_threads: 1,
        }
    }
}

impl Default for DeviceSelection {
    fn default() -> Self {
        Self::cpu_default()
    }
}

/// The runtime health of a model.
///
/// Health is known eagerly at add-on start (`inference-models.md` §4.1): a
/// model that fails to load or warm up reports [`ModelHealth::Unavailable`] and
/// its epochs are absent, never fabricated. Only a [`ModelHealth::Warm`] model
/// serves.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelHealth {
    /// The model is loading or warming up.
    Warming,
    /// The model is warm and may serve.
    Warm,
    /// The model is serving but with a recorded degradation.
    Degraded {
        /// A human-readable reason for the degraded state.
        reason: String,
    },
    /// The model is not available and must not serve.
    Unavailable {
        /// The typed failure that made the model unavailable.
        ///
        /// Boxed so that [`ModelError::Unavailable`] and this variant do not
        /// form an infinitely sized type.
        error: Box<ModelError>,
    },
}

/// The seam's typed error kind.
///
/// Every failure returns `Err`. There is **no** default, zeroed, last-known-good
/// or "best guess" output: a failed request produces no `Pose`, `VitalsReading`
/// or `Detection` for that span (`inference-models.md` §1.6).
///
/// Non-finite output is [`ModelError::NumericFault`], never coerced into a
/// claim; input `NaN` is permitted only where `signal.md` defines it.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ModelError {
    /// The model has not been loaded.
    #[error("model not loaded: {0}")]
    NotLoaded(ModelId),
    /// The model has loaded but has not completed warm-up.
    #[error("model not warmed up: {0}")]
    NotWarmedUp(ModelId),
    /// The on-disk weights digest does not match the model card.
    #[error("model digest mismatch: {model}")]
    DigestMismatch {
        /// The model whose digest disagreed.
        model: ModelId,
    },
    /// The card pins a contract version other than the runtime's.
    #[error("contract version mismatch: want {want:?}, got {got:?}")]
    ContractMismatch {
        /// The contract version the runtime expects.
        want: ContractVersion,
        /// The contract version the card declares.
        got: ContractVersion,
    },
    /// A licence or witness declaration is missing or internally conflicting.
    #[error("licence/witness declaration missing or conflicting")]
    LicenceMismatch {
        /// What was missing or conflicting.
        detail: String,
    },
    /// The request batch was empty.
    #[error("empty request batch")]
    EmptyBatch,
    /// An input tensor's shape is not the shape the graph requires.
    #[error("unsupported input shape for {name}: expected {expected:?}, got {got:?}")]
    UnsupportedInputShape {
        /// The graph input name.
        name: &'static str,
        /// The shape the graph requires.
        expected: Vec<Dim>,
        /// The shape supplied.
        got: Vec<usize>,
    },
    /// An input tensor's dtype is not the dtype the graph requires.
    #[error("dtype mismatch for {name}: expected {expected:?}, got {got:?}")]
    DtypeMismatch {
        /// The graph input name.
        name: &'static str,
        /// The dtype the graph requires.
        expected: DType,
        /// The dtype supplied.
        got: DType,
    },
    /// A feature vector is structurally malformed.
    #[error("feature vector malformed: {0}")]
    FeatureMalformed(String),
    /// The selected execution device is unavailable.
    #[error("device unavailable: {device:?} ({detail})")]
    DeviceUnavailable {
        /// The device that was requested.
        device: DeviceSelection,
        /// Why the device is unavailable.
        detail: String,
    },
    /// Building the inference engine failed.
    #[error("engine build failed: {0}")]
    EngineBuild(String),
    /// Inference failed at a named stage.
    #[error("inference failed at {stage}: {detail}")]
    InferenceFailed {
        /// The stage at which inference failed.
        stage: String,
        /// The underlying detail.
        detail: String,
    },
    /// An output tensor's shape is not the shape the graph declares.
    #[error("output shape mismatch for {name}: expected {expected:?}, got {got:?}")]
    OutputShapeMismatch {
        /// The graph output name.
        name: &'static str,
        /// The shape the graph declares.
        expected: Vec<Dim>,
        /// The shape observed.
        got: Vec<usize>,
    },
    /// An output element is `NaN` or infinite.
    #[error("non-finite output for {name} at {index}")]
    NumericFault {
        /// The graph output name.
        name: &'static str,
        /// The flat element index of the offending element.
        index: usize,
    },
    /// Inference exceeded its deadline.
    #[error("inference timeout at {stage} after {ms} ms")]
    Timeout {
        /// The stage that timed out.
        stage: String,
        /// The deadline, in milliseconds.
        ms: u64,
    },
    /// The model is unavailable at the current health.
    #[error("model unavailable: {0:?}")]
    Unavailable(ModelHealth),
    /// An internal error that has no more specific variant.
    #[error("internal error: {0}")]
    Internal(String),
}
