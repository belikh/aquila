//! ONNX Runtime adapters, gated behind the `onnx` feature (off by default).
//!
//! `inference-models.md` §3.2 recommends `ort` (ONNX Runtime), CPU execution
//! provider, version-pinned and statically bundled. The `ort` dependency is
//! deliberately **not** pulled yet, so this module exists, compiles and fails
//! **closed**: [`OnnxPoseModel::load`] and [`OnnxVitalsModel::load`] validate
//! the card and then return [`ModelError::EngineBuild`], and their `infer`
//! methods return [`ModelError::Unavailable`]. No output is ever fabricated.
//!
//! When `ort` is provisioned, the bodies here carry the remaining work:
//!
//! * hash `model_bytes` and compare to `card.onnx_sha256` (need a digest),
//! * build the engine under the selected [`DeviceSelection`],
//! * warm up with the committed `B = 1` and `B = max_batch_size` fixtures,
//! * map each [`FeatureVector`](crate::FeatureVector) to the declared tensor
//!   shapes, run the graph, post-process to the contract types, and raise
//!   `NumericFault` on any non-finite output.
//!
//! None of that is attempted here: a half-wired adapter that returned
//! plausible-looking tensors would be worse than one that refuses to run.

use aquila_types::VitalsReading;

use crate::error::{DeviceSelection, ModelError, ModelHealth};
use crate::model::{ModelCard, ModelId};
use crate::seam::{PoseModel, PoseOutcome, PoseRequest, VitalsModel, VitalsRequest};

/// The production pose adapter over ONNX Runtime.
///
/// Construction always fails closed until `ort` is provisioned; see the module
/// documentation.
pub struct OnnxPoseModel {
    id: ModelId,
    card: ModelCard,
    device: DeviceSelection,
}

impl OnnxPoseModel {
    /// Load a pose model from its card and weight bytes.
    ///
    /// The card is validated and an untrained (`Imported`) seed is refused;
    /// then engine construction fails closed because `ort` is not a dependency
    /// in this build.
    pub fn load(
        card: ModelCard,
        _model_bytes: &[u8],
        _device: DeviceSelection,
    ) -> Result<Self, ModelError> {
        card.validate()?;
        if !card.is_servable() {
            return Err(ModelError::Unavailable(ModelHealth::Unavailable {
                error: Box::new(ModelError::LicenceMismatch {
                    detail: "weights_trained=Imported: an untrained seed may not serve".to_string(),
                }),
            }));
        }
        Err(ModelError::EngineBuild(
            "the ONNX Runtime adapter is not implemented; `ort` is not yet a dependency \
             (TODO in src/onnx.rs)"
                .to_string(),
        ))
    }

    /// The card this adapter was configured with.
    #[must_use]
    pub fn card(&self) -> &ModelCard {
        &self.card
    }

    /// The device selection this adapter was configured with.
    #[must_use]
    pub fn device(&self) -> &DeviceSelection {
        &self.device
    }
}

impl PoseModel for OnnxPoseModel {
    fn id(&self) -> &ModelId {
        &self.id
    }

    fn infer(&self, requests: &[PoseRequest]) -> Result<Vec<PoseOutcome>, ModelError> {
        if requests.is_empty() {
            return Err(ModelError::EmptyBatch);
        }
        Err(ModelError::Unavailable(ModelHealth::Unavailable {
            error: Box::new(ModelError::EngineBuild(
                "ONNX Runtime is not wired in this build".to_string(),
            )),
        }))
    }
}

/// The production vitals adapter over ONNX Runtime.
///
/// Construction always fails closed until `ort` is provisioned; see the module
/// documentation.
pub struct OnnxVitalsModel {
    id: ModelId,
    card: ModelCard,
    device: DeviceSelection,
}

impl OnnxVitalsModel {
    /// Load a vitals model from its card and weight bytes.
    ///
    /// The card is validated and an untrained (`Imported`) seed is refused;
    /// then engine construction fails closed because `ort` is not a dependency
    /// in this build.
    pub fn load(
        card: ModelCard,
        _model_bytes: &[u8],
        _device: DeviceSelection,
    ) -> Result<Self, ModelError> {
        card.validate()?;
        if !card.is_servable() {
            return Err(ModelError::Unavailable(ModelHealth::Unavailable {
                error: Box::new(ModelError::LicenceMismatch {
                    detail: "weights_trained=Imported: an untrained seed may not serve".to_string(),
                }),
            }));
        }
        Err(ModelError::EngineBuild(
            "the ONNX Runtime adapter is not implemented; `ort` is not yet a dependency \
             (TODO in src/onnx.rs)"
                .to_string(),
        ))
    }

    /// The card this adapter was configured with.
    #[must_use]
    pub fn card(&self) -> &ModelCard {
        &self.card
    }

    /// The device selection this adapter was configured with.
    #[must_use]
    pub fn device(&self) -> &DeviceSelection {
        &self.device
    }
}

impl VitalsModel for OnnxVitalsModel {
    fn id(&self) -> &ModelId {
        &self.id
    }

    fn infer(&self, requests: &[VitalsRequest]) -> Result<Vec<VitalsReading>, ModelError> {
        if requests.is_empty() {
            return Err(ModelError::EmptyBatch);
        }
        Err(ModelError::Unavailable(ModelHealth::Unavailable {
            error: Box::new(ModelError::EngineBuild(
                "ONNX Runtime is not wired in this build".to_string(),
            )),
        }))
    }
}
