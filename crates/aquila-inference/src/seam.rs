//! The `PoseModel` / `VitalsModel` seam.
//!
//! The seam speaks only domain types from the L0 contract crate
//! (`aquila-types`) and this crate's feature carrier. No `ort`, `ndarray`,
//! `candle` or `burn` type appears in the interface, so a future engine swap
//! touches no caller (`inference-models.md` §1.1).
//!
//! Each trait exposes exactly one batch method. Batching is expressed
//! slice-in, `Vec`-out so the dynamic `B` axis is real while remaining one
//! method per trait (`inference-models.md` §1.3).

use aquila_types::{
    Detection, DetectionScope, FeatureVector, NodeId, Pose, SubjectId, TimeSpan, VitalsReading,
};

use crate::error::ModelError;
use crate::model::ModelId;

/// The default cap on requests in one batch (`inference-models.md` §1.4, §4.3).
pub const DEFAULT_MAX_BATCH_SIZE: usize = 8;

/// One unit of pose work: a feature set, the scope it came from, and the
/// topology facts the head needs to emit a contract-shaped result.
#[derive(Debug, Clone, PartialEq)]
pub struct PoseRequest {
    /// The scope the work applies to: the home or a room (the zone in v1).
    pub scope: DetectionScope,
    /// The contributing nodes; non-empty and ordered, copied into Detections.
    pub contributor_nodes: Vec<NodeId>,
    /// The core-assigned subject slot, if any.
    pub subject: Option<SubjectId>,
    /// The span the work covers.
    pub span: TimeSpan,
    /// The features from `aquila-signal`.
    pub features: FeatureVector,
}

impl PoseRequest {
    /// Validate the request against the seam's structural contract.
    ///
    /// This is the same check a real adapter applies before inference, so a
    /// shape bug fails identically in tests and production
    /// (`inference-models.md` §1.7). A malformed request is
    /// [`ModelError::FeatureMalformed`], never a silently accepted row.
    pub fn validate(&self) -> Result<(), ModelError> {
        validate_common(&self.scope, &self.span, &self.contributor_nodes, &self.features)
    }
}

/// Pose plus the shared-trunk auxiliary heads: one inference call, one bundle.
///
/// `None` is an honest "no claim this span", not a zero or a guess
/// (`inference-models.md` §1.3).
#[derive(Debug, Clone, PartialEq)]
pub struct PoseOutcome {
    /// The pose, or `None` when there is no pose claim for this span.
    pub pose: Option<Pose>,
    /// The presence head, or `None`.
    pub presence: Option<Detection>,
    /// The count head, or `None`.
    pub count: Option<Detection>,
}

/// The pose seam. Object-safe, so callers hold `Arc<dyn PoseModel>`.
pub trait PoseModel: Send + Sync {
    /// The content-addressed identity of the weights behind this model.
    fn id(&self) -> &ModelId;

    /// Infer a batch of pose requests.
    ///
    /// `requests` is non-empty; the result returns one outcome per request, in
    /// the same order. An empty batch is [`ModelError::EmptyBatch`]; any failure
    /// yields no output for the batch (`inference-models.md` §4.4).
    fn infer(&self, requests: &[PoseRequest]) -> Result<Vec<PoseOutcome>, ModelError>;
}

/// One unit of vitals work; the vitals analogue of [`PoseRequest`].
#[derive(Debug, Clone, PartialEq)]
pub struct VitalsRequest {
    /// The scope the work applies to: the home or a room (the zone in v1).
    pub scope: DetectionScope,
    /// The contributing nodes; non-empty and ordered.
    pub contributor_nodes: Vec<NodeId>,
    /// The core-assigned subject slot, if any.
    pub subject: Option<SubjectId>,
    /// The span the work covers.
    pub span: TimeSpan,
    /// The features from `aquila-signal`.
    pub features: FeatureVector,
}

impl VitalsRequest {
    /// Validate the request against the seam's structural contract.
    ///
    /// The same check a real adapter applies before inference; see
    /// [`PoseRequest::validate`].
    pub fn validate(&self) -> Result<(), ModelError> {
        validate_common(&self.scope, &self.span, &self.contributor_nodes, &self.features)
    }
}

/// The vitals seam. Object-safe, so callers hold `Arc<dyn VitalsModel>`.
pub trait VitalsModel: Send + Sync {
    /// The content-addressed identity of the weights behind this model.
    fn id(&self) -> &ModelId;

    /// Infer a batch of vitals requests.
    ///
    /// `requests` is non-empty; the result returns one reading per request, in
    /// the same order. An empty batch is [`ModelError::EmptyBatch`]; any failure
    /// yields no output for the batch (`inference-models.md` §4.4).
    fn infer(&self, requests: &[VitalsRequest]) -> Result<Vec<VitalsReading>, ModelError>;
}

/// The structural checks shared by both request types.
///
/// A model card's declared specs pin the tensor shapes; this function only
/// checks the parts of the request the seam fixes regardless of the graph: a
/// valid scope and span, a non-empty set of provisioned contributors, and a
/// present feature vector.
fn validate_common(
    scope: &DetectionScope,
    span: &TimeSpan,
    contributor_nodes: &[NodeId],
    features: &FeatureVector,
) -> Result<(), ModelError> {
    scope
        .validate()
        .map_err(|error| ModelError::FeatureMalformed(error.to_string()))?;
    span.validate()
        .map_err(|error| ModelError::FeatureMalformed(error.to_string()))?;
    if contributor_nodes.is_empty() {
        return Err(ModelError::FeatureMalformed(
            "contributor_nodes must be non-empty".to_string(),
        ));
    }
    for node in contributor_nodes {
        node.validate_provisioned()
            .map_err(|error| ModelError::FeatureMalformed(error.to_string()))?;
    }
    if features.is_empty() {
        return Err(ModelError::FeatureMalformed(
            "the feature vector is empty".to_string(),
        ));
    }
    Ok(())
}
