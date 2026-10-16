//! Deterministic stub models behind the `PoseModel`/`VitalsModel` seams.
//!
//! The pipeline, window and fusion tests never depend on ONNX: these fakes
//! return an honest "no claim this span" rather than a fabricated pose or rate
//! (`inference-models.md` §1.6). They implement the same traits as the real
//! adapters, so they cannot drift from the seam (`testing.md` §1).

use aquila_sensing::{
    ModelError, ModelId, PoseModel, PoseOutcome, PoseRequest, VitalsModel, VitalsRequest,
};
use aquila_types::{DetectionScope, VitalsReading, VitalsStatus, Witness};

/// A pose model that claims nothing.
#[derive(Debug, Clone)]
pub struct FakePoseModel {
    id: ModelId,
}

impl FakePoseModel {
    /// Construct the fake with a fixed, content-addressed identity.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: ModelId::from_digest([0u8; 32]),
        }
    }
}

impl Default for FakePoseModel {
    fn default() -> Self {
        Self::new()
    }
}

impl PoseModel for FakePoseModel {
    fn id(&self) -> &ModelId {
        &self.id
    }

    fn infer(&self, requests: &[PoseRequest]) -> Result<Vec<PoseOutcome>, ModelError> {
        if requests.is_empty() {
            return Err(ModelError::EmptyBatch);
        }
        Ok(requests
            .iter()
            .map(|_| PoseOutcome {
                pose: None,
                presence: None,
                count: None,
            })
            .collect())
    }
}

/// A vitals model that claims nothing.
#[derive(Debug, Clone)]
pub struct FakeVitalsModel {
    id: ModelId,
}

impl FakeVitalsModel {
    /// Construct the fake with a fixed, content-addressed identity.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: ModelId::from_digest([0u8; 32]),
        }
    }
}

impl Default for FakeVitalsModel {
    fn default() -> Self {
        Self::new()
    }
}

impl VitalsModel for FakeVitalsModel {
    fn id(&self) -> &ModelId {
        &self.id
    }

    fn infer(&self, requests: &[VitalsRequest]) -> Result<Vec<VitalsReading>, ModelError> {
        if requests.is_empty() {
            return Err(ModelError::EmptyBatch);
        }
        Ok(requests
            .iter()
            .map(|request| {
                let room = match &request.scope {
                    DetectionScope::Zone(room) => room.clone(),
                    DetectionScope::Home(home) => home.clone(),
                };
                VitalsReading {
                    subject: request.subject,
                    room,
                    span: request.span,
                    breathing: None,
                    heart_rate: None,
                    status: VitalsStatus::NoSubject,
                    witness: Witness::Claimed,
                }
            })
            .collect())
    }
}
