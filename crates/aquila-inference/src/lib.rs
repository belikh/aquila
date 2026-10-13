#![forbid(unsafe_code)]

//! `aquila-inference` — the `PoseModel` / `VitalsModel` seam.
//!
//! This crate is level 2 in the workspace dependency graph: it depends only on
//! `aquila-types` (L0). It owns the model-inference seam and hides the engine,
//! tensor preparation, keypoint post-processing and model version pinning
//! (`inference-models.md` §1.1). No `ort`, `ndarray`, `candle` or `burn` type
//! appears in its public interface.
//!
//! # Shape of the seam
//!
//! Two object-safe traits, [`PoseModel`] and [`VitalsModel`], each expose one
//! batch-slice method ([`PoseModel::infer`] / [`VitalsModel::infer`]). They
//! speak the L0 contract types — [`FeatureVector`](crate::FeatureVector) in,
//! [`Pose`](aquila_types::Pose) / [`VitalsReading`](aquila_types::VitalsReading)
//! and presence/count [`Detection`](aquila_types::Detection)s out — so a caller
//! never names an engine type. The add-on composition root constructs the
//! concrete adapters and injects them as trait objects.
//!
//! # Never fabricate
//!
//! Every failure returns [`ModelError`]; there is no zeroed, default or
//! last-known-good output. Absence is expressed through the contract itself: a
//! pose with `confidence == 0.0`, a vitals estimate of `None`. A failure to run
//! and an honest absence are different things, and this crate keeps them apart
//! (`inference-models.md` §1.6).
//!
//! # Model provenance
//!
//! A model is content-addressed by [`ModelId`], and its [`ModelCard`] records
//! `weights_trained`, dataset references, the weight licence and the derived
//! [`Witness`](aquila_types::Witness). `Imported` seed weights alone are not a
//! working Aquila model and may not serve (`inference-models.md` §2.2, §2.6).
//!
//! # Features
//!
//! The ONNX Runtime adapters ([`OnnxPoseModel`] / [`OnnxVitalsModel`]) are
//! gated behind the `onnx` feature, **off by default**. The `ort` dependency is
//! intentionally not pulled yet, so the gated adapters fail closed. `cuda` and
//! `openvino` gate the corresponding [`DeviceSelection`] variants and are also
//! off by default.
//!
//! # Prose
//!
//! Australian English throughout (licence, behaviour, serialisation,
//! normalise).

mod error;
mod feature;
mod model;
mod seam;

#[cfg(feature = "onnx")]
mod onnx;

#[cfg(test)]
mod tests;

pub use error::{DType, DeviceSelection, Dim, ModelError, ModelHealth};
pub use feature::FeatureVector;
pub use model::{
    check_tensor_dims, DatasetRecord, ModelCard, ModelId, ModelTask, SeedWeights, TensorSpec,
    WeightsTrained,
};
pub use seam::{
    PoseModel, PoseOutcome, PoseRequest, VitalsModel, VitalsRequest, DEFAULT_MAX_BATCH_SIZE,
};

#[cfg(feature = "onnx")]
pub use onnx::{OnnxPoseModel, OnnxVitalsModel};
