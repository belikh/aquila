//! Model identity, provenance and the model card.
//!
//! A model is content-addressed: [`ModelId`] is the lower-case hex SHA-256 of
//! the `.onnx` bytes (`inference-models.md` §2.5). The card records the
//! provenance the loader needs to refuse an untethered number: how the weights
//! were trained, which datasets and licences they derive from, the contract
//! version they were trained against, and the evidence grade the card supports.
//! The card is required for admission and its `witness` is *derived* from the
//! ledger, never asserted (`inference-models.md` §2.6).

use core::fmt;

use serde::{Deserialize, Serialize};

use aquila_types::{ContractVersion, Split, ValidationError, Witness, CONTRACT_VERSION};

use crate::error::{DType, Dim, ModelError};

/// A content-addressed model identity: the lower-case hex SHA-256 of the
/// `.onnx` weights bytes.
///
/// The identity is echoed into the ledger so a metric is always tethered to the
/// exact weights that produced it (`inference-models.md` §1.3). This crate does
/// not hash: [`ModelId::new`] validates a hex digest supplied by the loader and
/// [`ModelId::from_digest`] encodes a raw digest, so no hashing dependency is
/// pulled into the seam.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelId(String);

impl ModelId {
    /// Construct a model identity from a 64-character lower-case hex digest.
    ///
    /// Upper-case hex, a wrong length or a non-hex character is rejected.
    pub fn new(hex: impl Into<String>) -> Result<Self, ValidationError> {
        let hex = hex.into();
        let is_lower_hex = hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if hex.len() == 64 && is_lower_hex {
            Ok(Self(hex))
        } else {
            Err(ValidationError::Range {
                field: "ModelId",
                detail: "expected 64 lower-case hexadecimal characters".to_string(),
            })
        }
    }

    /// Encode a raw 32-byte digest as a model identity.
    #[must_use]
    pub fn from_digest(digest: [u8; 32]) -> Self {
        use fmt::Write as _;
        let mut hex = String::with_capacity(64);
        for byte in digest {
            // Writing into a `String` cannot fail.
            let _ = write!(hex, "{byte:02x}");
        }
        Self(hex)
    }

    /// Borrow the hex digest as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Which seam a model serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTask {
    /// The model serves the [`PoseModel`](crate::PoseModel) seam.
    Pose,
    /// The model serves the [`VitalsModel`](crate::VitalsModel) seam.
    Vitals,
}

/// How a model's weights were trained.
///
/// This distinguishes provenance honestly (`inference-models.md` §2.2). An
/// [`WeightsTrained::Imported`] seed is not trained by Aquila and is never a
/// working model: it is `Claimed` until measured, and it may not serve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightsTrained {
    /// The ruvnet NC seed, imported verbatim and not trained by Aquila.
    Imported,
    /// Fine-tuned from the imported seed on Aquila data.
    FineTunedFromSeed,
    /// Trained from scratch, with no seed.
    TrainedFromScratch,
}

/// The upstream seed weights a model was initialised from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedWeights {
    /// The upstream reference, for example a repository and revision.
    pub reference: String,
    /// The exact SHA-256 of the seed weights file, recorded verbatim.
    pub sha256: [u8; 32],
    /// The exact licence string of the seed weights, recorded verbatim.
    pub licence: String,
}

/// One dataset a model was trained on or evaluated against.
///
/// Every entry records the exact licence string and the strongest
/// [`Witness`] the data can support; a model may not claim a grade its data
/// cannot support (`inference-models.md` §2.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetRecord {
    /// The dataset name.
    pub name: String,
    /// The dataset version or split digest.
    pub version: String,
    /// The split used.
    pub split: Split,
    /// The exact upstream licence string, recorded verbatim.
    pub licence: String,
    /// The SHA-256 of the dataset or split used.
    pub sha256: [u8; 32],
    /// The strongest grade the data can support.
    pub witness: Witness,
}

/// A declared tensor input or output of the graph: a name, a dtype and dims.
///
/// The names and shapes are part of the model contract
/// (`inference-models.md` §1.4, §2.4). Validate a supplied shape against a spec
/// with [`check_tensor_dims`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TensorSpec {
    /// The tensor name, exactly as exported.
    pub name: String,
    /// The tensor element type.
    pub dtype: DType,
    /// The tensor shape, with the dynamic batch axis as [`Dim::Batch`].
    pub dims: Vec<Dim>,
}

/// Check a supplied tensor shape against a [`TensorSpec`].
///
/// The batch axis accepts any positive extent; every fixed axis must match
/// exactly. A mismatch is a typed [`ModelError::UnsupportedInputShape`], never a
/// silent resize or pad (`inference-models.md` §1.4).
pub fn check_tensor_dims(
    name: &'static str,
    spec: &TensorSpec,
    got: &[usize],
) -> Result<(), ModelError> {
    let expected = &spec.dims;
    let shape_error = || ModelError::UnsupportedInputShape {
        name,
        expected: expected.clone(),
        got: got.to_vec(),
    };
    if expected.len() != got.len() {
        return Err(shape_error());
    }
    for (dim, actual) in expected.iter().zip(got) {
        match dim {
            // The dynamic batch axis may be any positive extent; zero is the
            // empty batch, which is rejected as `EmptyBatch` upstream.
            Dim::Batch => {
                if *actual == 0 {
                    return Err(shape_error());
                }
            }
            Dim::Fixed(n) => {
                if n != actual {
                    return Err(shape_error());
                }
            }
        }
    }
    Ok(())
}

/// The model card: the provenance record required for admission.
///
/// A model card records identity, task, the contract it was trained against,
/// how its weights were made, the datasets and licences behind them, the exact
/// graph input and output specs, and the `Witness` the evidence supports. The
/// loader refuses a card whose licence or witness declaration is missing or
/// conflicting ([`ModelError::LicenceMismatch`]) or whose pinned contract
/// disagrees with the runtime ([`ModelError::ContractMismatch`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelCard {
    /// The model name.
    pub name: String,
    /// The semantic version of the model, independent of the code version.
    pub semver: String,
    /// The SHA-256 of the `.onnx` bytes; must equal the file digest.
    pub onnx_sha256: [u8; 32],
    /// Which seam the model serves.
    pub task: ModelTask,
    /// The contract version the model was trained against.
    pub trained_contract_version: ContractVersion,
    /// How the weights were trained.
    pub weights_trained: WeightsTrained,
    /// The upstream seed weights, or `None` when none were used.
    pub seed_weights: Option<SeedWeights>,
    /// The datasets behind the weights.
    pub datasets: Vec<DatasetRecord>,
    /// The weights licence string, recorded verbatim (may be NC).
    pub licence: String,
    /// Whether commercial use is permitted. `false` for v1.
    pub commercial_use: bool,
    /// The strongest evidence grade the card supports, derived from the ledger.
    pub witness: Witness,
    /// The declared graph inputs.
    pub input_spec: Vec<TensorSpec>,
    /// The declared graph outputs.
    pub output_spec: Vec<TensorSpec>,
    /// The ONNX opset the graph was exported at.
    pub opset: u32,
    /// The names of the dynamic axes; only the batch axis is dynamic in v1.
    pub dynamic_axes: Vec<String>,
    /// The digest of the exact `FeatureVector`→tensor projection.
    pub feature_map_digest: [u8; 32],
}

impl ModelCard {
    /// The content-addressed identity of the model this card describes.
    #[must_use]
    pub fn model_id(&self) -> ModelId {
        ModelId::from_digest(self.onnx_sha256)
    }

    /// Whether the weights are trained by Aquila and so may serve.
    ///
    /// An [`WeightsTrained::Imported`] seed alone is never a working model.
    #[must_use]
    pub const fn is_servable(&self) -> bool {
        !matches!(self.weights_trained, WeightsTrained::Imported)
    }

    /// Validate the card's admission rules.
    ///
    /// * `name` and `semver` must be non-empty.
    /// * `licence` must be non-empty; a missing licence is
    ///   [`ModelError::LicenceMismatch`].
    /// * An `Imported` seed may only carry [`Witness::Claimed`]; a stronger
    ///   grade on untrained weights is a conflicting witness declaration.
    /// * `trained_contract_version` must equal the runtime's
    ///   [`CONTRACT_VERSION`].
    /// * The input and output specs must be non-empty and the opset non-zero.
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.name.is_empty() || self.semver.is_empty() {
            return Err(ModelError::Internal(
                "model card must carry a name and a semver".to_string(),
            ));
        }
        if self.licence.is_empty() {
            return Err(ModelError::LicenceMismatch {
                detail: "the weights licence string is missing".to_string(),
            });
        }
        if matches!(self.weights_trained, WeightsTrained::Imported)
            && self.witness != Witness::Claimed
        {
            return Err(ModelError::LicenceMismatch {
                detail: "untrained Imported seed may only carry Witness::Claimed".to_string(),
            });
        }
        if self.trained_contract_version != CONTRACT_VERSION {
            return Err(ModelError::ContractMismatch {
                want: CONTRACT_VERSION,
                got: self.trained_contract_version,
            });
        }
        if self.input_spec.is_empty() || self.output_spec.is_empty() {
            return Err(ModelError::Internal(
                "model card must declare its input and output specs".to_string(),
            ));
        }
        if self.opset == 0 {
            return Err(ModelError::Internal(
                "model card must declare a non-zero opset".to_string(),
            ));
        }
        Ok(())
    }
}
