//! Unit tests for the inference seam.
//!
//! The tests exercise the *trait contract* with a local, deterministic test
//! double defined here (not in `aquila-testkit`). They assert the typed error
//! surface, that a batch yields one outcome per request in order, and that
//! absence is expressed through the L0 contract (zero confidence, `None`
//! estimates) rather than fabricated numbers or a panic.

use aquila_types::{
    ContractVersion, Detection, DetectionKind, DetectionScope, DetectionValue, Estimate, Keypoint,
    KeypointId, NodeId, Pose, RoomId, Slug, SubjectId, TimeSpan, VitalsReading, VitalsStatus,
    Witness, CONTRACT_VERSION, KEYPOINT_COUNT,
};
use proptest::prelude::*;

use crate::error::{DType, Dim, ModelError, ModelHealth};
use aquila_types::FeatureVector;
use crate::model::{
    check_tensor_dims, DatasetRecord, ModelCard, ModelId, ModelTask, SeedWeights, TensorSpec,
    WeightsTrained,
};
use crate::seam::{PoseModel, PoseOutcome, PoseRequest, VitalsModel, VitalsRequest};

// --- fixtures -------------------------------------------------------------

/// The COCO-17 order, written out so tests build schema-correct poses without
/// relying on transmuting discriminants.
const ALL_KEYPOINTS: [KeypointId; KEYPOINT_COUNT] = [
    KeypointId::Nose,
    KeypointId::LeftEye,
    KeypointId::RightEye,
    KeypointId::LeftEar,
    KeypointId::RightEar,
    KeypointId::LeftShoulder,
    KeypointId::RightShoulder,
    KeypointId::LeftElbow,
    KeypointId::RightElbow,
    KeypointId::LeftWrist,
    KeypointId::RightWrist,
    KeypointId::LeftHip,
    KeypointId::RightHip,
    KeypointId::LeftKnee,
    KeypointId::RightKnee,
    KeypointId::LeftAnkle,
    KeypointId::RightAnkle,
];

fn model_id() -> ModelId {
    ModelId::new("0".repeat(64)).expect("a 64-character lower-case hex id is valid")
}

fn room() -> RoomId {
    Slug::new("kitchen").expect("non-empty slug")
}

fn node(value: u16) -> NodeId {
    NodeId::new(value)
}

fn span() -> TimeSpan {
    TimeSpan {
        start_mono_ns: 1_000,
        end_mono_ns: 2_000,
        start_unix_ms: 1_700_000_000_000,
        end_unix_ms: 1_700_000_001_000,
    }
}

fn features() -> FeatureVector {
    FeatureVector::new(vec![0.1, 0.2, 0.3, 0.4])
}

fn sample_pose(room: &RoomId, subject: SubjectId, present: bool) -> Pose {
    let keys: [Keypoint; KEYPOINT_COUNT] = core::array::from_fn(|i| {
        if present {
            Keypoint {
                id: ALL_KEYPOINTS[i],
                x_m: i as f32 * 0.1,
                y_m: i as f32 * 0.05,
                z_m: None,
                confidence: 0.9,
            }
        } else {
            // Absence through the contract: no estimate, never invented.
            Keypoint {
                id: ALL_KEYPOINTS[i],
                x_m: f32::NAN,
                y_m: f32::NAN,
                z_m: None,
                confidence: 0.0,
            }
        }
    });
    Pose {
        subject,
        room: room.clone(),
        t_unix_ms: 1_700_000_000_000,
        keys,
        score: if present { 0.8 } else { 0.0 },
        witness: Witness::Measured,
    }
}

fn sample_presence(
    scope: &DetectionScope,
    contributors: &[NodeId],
    span: TimeSpan,
    probability: f32,
) -> Detection {
    Detection {
        scope: scope.clone(),
        span,
        kind: DetectionKind::Presence,
        confidence: 0.9,
        value: DetectionValue::Probability(probability),
        contributors: contributors.to_vec(),
        witness: Witness::Measured,
    }
}

fn sample_count(
    scope: &DetectionScope,
    contributors: &[NodeId],
    span: TimeSpan,
    count: u8,
) -> Detection {
    Detection {
        scope: scope.clone(),
        span,
        kind: DetectionKind::Count,
        confidence: 0.9,
        value: DetectionValue::Count(count),
        contributors: contributors.to_vec(),
        witness: Witness::Measured,
    }
}

fn pose_request(subject: Option<SubjectId>) -> PoseRequest {
    PoseRequest {
        scope: DetectionScope::Room(room()),
        contributor_nodes: vec![node(1), node(2)],
        subject,
        span: span(),
        features: features(),
    }
}

fn vitals_request(subject: Option<SubjectId>) -> VitalsRequest {
    VitalsRequest {
        scope: DetectionScope::Room(room()),
        contributor_nodes: vec![node(1), node(2)],
        subject,
        span: span(),
        features: features(),
    }
}

// --- local deterministic test double --------------------------------------

#[derive(Clone)]
enum TestMode {
    /// Emit contract-shaped output, with presence gated on `present`.
    Echo { present: bool },
    /// Fail every call with the given typed error.
    Fail(ModelError),
}

struct TestPoseModel {
    id: ModelId,
    mode: TestMode,
}

impl TestPoseModel {
    fn echo(id: ModelId, present: bool) -> Self {
        Self {
            id,
            mode: TestMode::Echo { present },
        }
    }

    fn failing(id: ModelId, error: ModelError) -> Self {
        Self {
            id,
            mode: TestMode::Fail(error),
        }
    }
}

impl PoseModel for TestPoseModel {
    fn id(&self) -> &ModelId {
        &self.id
    }

    fn infer(&self, requests: &[PoseRequest]) -> Result<Vec<PoseOutcome>, ModelError> {
        if requests.is_empty() {
            return Err(ModelError::EmptyBatch);
        }
        match &self.mode {
            TestMode::Fail(error) => Err(error.clone()),
            TestMode::Echo { present } => {
                let mut outcomes = Vec::with_capacity(requests.len());
                for request in requests {
                    request.validate()?;
                    let room = match &request.scope {
                        DetectionScope::Room(room) => Some(room.clone()),
                        DetectionScope::Home(_) => None,
                    };
                    let pose = match (room.as_ref(), request.subject) {
                        (Some(room), Some(subject)) if *present => {
                            Some(sample_pose(room, subject, true))
                        }
                        _ => None,
                    };
                    let presence = match (room.as_ref(), *present) {
                        (Some(_), true) => Some(sample_presence(
                            &request.scope,
                            &request.contributor_nodes,
                            request.span,
                            0.9,
                        )),
                        _ => None,
                    };
                    let count = if *present {
                        Some(sample_count(
                            &request.scope,
                            &request.contributor_nodes,
                            request.span,
                            1,
                        ))
                    } else {
                        None
                    };
                    outcomes.push(PoseOutcome {
                        pose,
                        presence,
                        count,
                    });
                }
                Ok(outcomes)
            }
        }
    }
}

#[derive(Clone)]
enum TestVitalsMode {
    Echo {
        breathing: Option<Estimate>,
        heart_rate: Option<Estimate>,
        status: VitalsStatus,
    },
    Fail(ModelError),
}

struct TestVitalsModel {
    id: ModelId,
    mode: TestVitalsMode,
}

impl VitalsModel for TestVitalsModel {
    fn id(&self) -> &ModelId {
        &self.id
    }

    fn infer(&self, requests: &[VitalsRequest]) -> Result<Vec<VitalsReading>, ModelError> {
        if requests.is_empty() {
            return Err(ModelError::EmptyBatch);
        }
        match &self.mode {
            TestVitalsMode::Fail(error) => Err(error.clone()),
            TestVitalsMode::Echo {
                breathing,
                heart_rate,
                status,
            } => {
                let mut readings = Vec::with_capacity(requests.len());
                for request in requests {
                    request.validate()?;
                    let room = match &request.scope {
                        DetectionScope::Room(room) => room.clone(),
                        DetectionScope::Home(_) => {
                            // Vitals are room-scoped in v1: a reading carries a
                            // room. A home-scoped request is malformed here.
                            return Err(ModelError::FeatureMalformed(
                                "vitals requires a room scope".to_string(),
                            ));
                        }
                    };
                    readings.push(VitalsReading {
                        subject: request.subject,
                        room,
                        span: request.span,
                        breathing: *breathing,
                        heart_rate: *heart_rate,
                        status: *status,
                        witness: Witness::Measured,
                    });
                }
                Ok(readings)
            }
        }
    }
}

// --- trait contract -------------------------------------------------------

#[test]
fn empty_batch_is_typed_for_pose() {
    let model = TestPoseModel::echo(model_id(), true);
    assert!(matches!(model.infer(&[]), Err(ModelError::EmptyBatch)));
}

#[test]
fn empty_batch_is_typed_for_vitals() {
    let model = TestVitalsModel {
        id: model_id(),
        mode: TestVitalsMode::Echo {
            breathing: Some(Estimate {
                value: 15.0,
                confidence: 0.8,
            }),
            heart_rate: None,
            status: VitalsStatus::Ok,
        },
    };
    assert!(matches!(model.infer(&[]), Err(ModelError::EmptyBatch)));
}

#[test]
fn missing_model_reports_not_loaded() {
    let id = model_id();
    let model = TestPoseModel::failing(id.clone(), ModelError::NotLoaded(id.clone()));
    let error = model
        .infer(&[pose_request(Some(SubjectId::new(1)))])
        .expect_err("a missing model must fail");
    assert!(matches!(error, ModelError::NotLoaded(found) if found == id));
}

#[test]
fn untrained_model_reports_unavailable() {
    let model = TestPoseModel::failing(
        model_id(),
        ModelError::Unavailable(ModelHealth::Unavailable {
            error: Box::new(ModelError::Internal(
                "weights_trained=Imported: an untrained seed may not serve".to_string(),
            )),
        }),
    );
    let error = model
        .infer(&[pose_request(Some(SubjectId::new(1)))])
        .expect_err("an untrained model must fail");
    assert!(matches!(
        error,
        ModelError::Unavailable(ModelHealth::Unavailable { .. })
    ));
}

#[test]
fn a_failing_batch_yields_no_output() {
    let model = TestVitalsModel {
        id: model_id(),
        mode: TestVitalsMode::Fail(ModelError::InferenceFailed {
            stage: "run".to_string(),
            detail: "synthetic failure".to_string(),
        }),
    };
    let result = model.infer(&[vitals_request(Some(SubjectId::new(1)))]);
    assert!(result.is_err());
}

#[test]
fn batch_shape_is_one_outcome_per_request_in_order() {
    let model = TestPoseModel::echo(model_id(), true);
    let requests: Vec<PoseRequest> = (0..3)
        .map(|i| pose_request(Some(SubjectId::new(i))))
        .collect();
    let outcomes = model.infer(&requests).expect("echo succeeds");
    assert_eq!(outcomes.len(), requests.len());
    for (i, outcome) in outcomes.iter().enumerate() {
        let pose = outcome.pose.as_ref().expect("a subject is present");
        assert_eq!(pose.subject, SubjectId::new(i as u32));
    }
}

#[test]
fn presence_and_count_copy_the_contributors() {
    let model = TestPoseModel::echo(model_id(), true);
    let request = pose_request(Some(SubjectId::new(1)));
    let outcomes = model
        .infer(std::slice::from_ref(&request))
        .expect("echo succeeds");
    let presence = outcomes[0].presence.as_ref().expect("presence present");
    let count = outcomes[0].count.as_ref().expect("count present");
    assert_eq!(presence.contributors, request.contributor_nodes);
    assert_eq!(count.contributors, request.contributor_nodes);
    assert!(presence.validate().is_ok());
    assert!(count.validate().is_ok());
}

// --- absence is expressed through the contract ----------------------------

#[test]
fn no_claim_emits_no_outputs() {
    let model = TestPoseModel::echo(model_id(), false);
    let outcomes = model
        .infer(&[pose_request(Some(SubjectId::new(1)))])
        .expect("echo succeeds");
    assert!(outcomes[0].pose.is_none());
    assert!(outcomes[0].presence.is_none());
    assert!(outcomes[0].count.is_none());
}

#[test]
fn absent_keypoints_use_zero_confidence_not_an_error() {
    // The contract's own representation of a missing keypoint is
    // `confidence == 0.0`; NaN coordinates are permitted and mean "no estimate".
    let pose = sample_pose(&room(), SubjectId::new(1), false);
    assert!(pose.validate().is_ok());
    assert!(pose.keys.iter().all(|key| key.confidence == 0.0));
    assert_eq!(pose.score, 0.0);
}

#[test]
fn vitals_without_signal_is_a_reading_not_an_error() {
    let model = TestVitalsModel {
        id: model_id(),
        mode: TestVitalsMode::Echo {
            breathing: None,
            heart_rate: None,
            status: VitalsStatus::InsufficientSignal,
        },
    };
    let readings = model
        .infer(&[vitals_request(Some(SubjectId::new(1)))])
        .expect("echo succeeds");
    assert!(readings[0].breathing.is_none());
    assert!(readings[0].heart_rate.is_none());
    assert!(readings[0].validate().is_ok());
}

#[test]
fn vitals_ok_reading_is_contract_valid() {
    let model = TestVitalsModel {
        id: model_id(),
        mode: TestVitalsMode::Echo {
            breathing: Some(Estimate {
                value: 15.0,
                confidence: 0.8,
            }),
            heart_rate: None,
            status: VitalsStatus::Ok,
        },
    };
    let readings = model
        .infer(&[vitals_request(Some(SubjectId::new(1)))])
        .expect("echo succeeds");
    assert!(readings[0].breathing.is_some());
    assert!(readings[0].validate().is_ok());
}

#[test]
fn vitals_home_scope_is_malformed() {
    let model = TestVitalsModel {
        id: model_id(),
        mode: TestVitalsMode::Echo {
            breathing: None,
            heart_rate: None,
            status: VitalsStatus::NoSubject,
        },
    };
    let mut request = vitals_request(Some(SubjectId::new(1)));
    request.scope = DetectionScope::Home(Slug::new("home").expect("non-empty slug"));
    assert!(matches!(
        model.infer(&[request]),
        Err(ModelError::FeatureMalformed(_))
    ));
}

// --- request validation ----------------------------------------------------

#[test]
fn empty_feature_vector_is_malformed() {
    let model = TestPoseModel::echo(model_id(), true);
    let mut request = pose_request(Some(SubjectId::new(1)));
    request.features = FeatureVector::new(Vec::new());
    assert!(matches!(
        model.infer(&[request]),
        Err(ModelError::FeatureMalformed(_))
    ));
}

#[test]
fn empty_contributors_are_malformed() {
    let model = TestPoseModel::echo(model_id(), true);
    let mut request = pose_request(Some(SubjectId::new(1)));
    request.contributor_nodes.clear();
    assert!(matches!(
        model.infer(&[request]),
        Err(ModelError::FeatureMalformed(_))
    ));
}

#[test]
fn unprovisioned_contributor_is_malformed() {
    let model = TestPoseModel::echo(model_id(), true);
    let mut request = pose_request(Some(SubjectId::new(1)));
    request.contributor_nodes = vec![NodeId::UNPROVISIONED];
    assert!(matches!(
        model.infer(&[request]),
        Err(ModelError::FeatureMalformed(_))
    ));
}

// --- model card and provenance --------------------------------------------

fn sample_card(weights: WeightsTrained, witness: Witness, contract: ContractVersion) -> ModelCard {
    ModelCard {
        name: "pose-v1".to_string(),
        semver: "1.0.0".to_string(),
        onnx_sha256: [0u8; 32],
        task: ModelTask::Pose,
        trained_contract_version: contract,
        weights_trained: weights,
        seed_weights: Some(SeedWeights {
            reference: "ruvnet/nc-seed@rev1".to_string(),
            sha256: [1u8; 32],
            licence: "CC-BY-NC-4.0".to_string(),
        }),
        datasets: vec![DatasetRecord {
            name: "MM-Fi".to_string(),
            version: "split-train-digest".to_string(),
            split: aquila_types::Split::Train,
            licence: "research-only".to_string(),
            sha256: [2u8; 32],
            witness: Witness::Measured,
        }],
        licence: "CC-BY-NC-4.0".to_string(),
        commercial_use: false,
        witness,
        input_spec: vec![TensorSpec {
            name: "amplitude".to_string(),
            dtype: DType::F32,
            dims: vec![Dim::Batch, Dim::Fixed(100), Dim::Fixed(64), Dim::Fixed(3)],
        }],
        output_spec: vec![TensorSpec {
            name: "keypoints".to_string(),
            dtype: DType::F32,
            dims: vec![Dim::Batch, Dim::Fixed(17), Dim::Fixed(3)],
        }],
        opset: 17,
        dynamic_axes: vec!["batch".to_string()],
        feature_map_digest: [3u8; 32],
    }
}

#[test]
fn imported_seed_may_only_carry_claimed() {
    let claimed = sample_card(
        WeightsTrained::Imported,
        Witness::Claimed,
        CONTRACT_VERSION,
    );
    assert!(claimed.validate().is_ok());
    assert!(!claimed.is_servable());

    let overclaimed = sample_card(
        WeightsTrained::Imported,
        Witness::Measured,
        CONTRACT_VERSION,
    );
    assert!(matches!(
        overclaimed.validate(),
        Err(ModelError::LicenceMismatch { .. })
    ));
}

#[test]
fn fine_tuned_model_is_servable() {
    let card = sample_card(
        WeightsTrained::FineTunedFromSeed,
        Witness::Measured,
        CONTRACT_VERSION,
    );
    assert!(card.is_servable());
    assert!(card.validate().is_ok());
}

#[test]
fn contract_mismatch_is_rejected() {
    let card = sample_card(
        WeightsTrained::FineTunedFromSeed,
        Witness::Measured,
        ContractVersion::new(2, 0),
    );
    assert!(matches!(
        card.validate(),
        Err(ModelError::ContractMismatch { .. })
    ));
}

#[test]
fn missing_licence_is_rejected() {
    let mut card = sample_card(
        WeightsTrained::FineTunedFromSeed,
        Witness::Measured,
        CONTRACT_VERSION,
    );
    card.licence = String::new();
    assert!(matches!(
        card.validate(),
        Err(ModelError::LicenceMismatch { .. })
    ));
}

#[test]
fn model_id_is_the_onnx_digest() {
    let mut card = sample_card(
        WeightsTrained::TrainedFromScratch,
        Witness::Measured,
        CONTRACT_VERSION,
    );
    card.onnx_sha256 = [0xab; 32];
    assert_eq!(card.model_id().as_str(), "ab".repeat(32));
    assert_eq!(card.model_id().to_string(), "ab".repeat(32));
}

// --- identity and tensor specs --------------------------------------------

#[test]
fn model_id_validation() {
    assert!(ModelId::new("a".repeat(64)).is_ok());
    assert!(ModelId::new("A".repeat(64)).is_err());
    assert!(ModelId::new("a".repeat(63)).is_err());
    assert!(ModelId::new("z".repeat(64)).is_err());
    let encoded = ModelId::from_digest([0u8; 32]);
    assert_eq!(encoded.as_str(), "0".repeat(64));
}

#[test]
fn tensor_shape_check_is_typed() {
    let spec = TensorSpec {
        name: "amplitude".to_string(),
        dtype: DType::F32,
        dims: vec![Dim::Batch, Dim::Fixed(100), Dim::Fixed(64), Dim::Fixed(3)],
    };
    assert!(check_tensor_dims("amplitude", &spec, &[1, 100, 64, 3]).is_ok());
    assert!(check_tensor_dims("amplitude", &spec, &[8, 100, 64, 3]).is_ok());
    assert!(matches!(
        check_tensor_dims("amplitude", &spec, &[1, 99, 64, 3]),
        Err(ModelError::UnsupportedInputShape { .. })
    ));
    assert!(matches!(
        check_tensor_dims("amplitude", &spec, &[1, 100, 64]),
        Err(ModelError::UnsupportedInputShape { .. })
    ));
}

// --- property: batch length is always preserved ---------------------------

proptest! {
    #[test]
    fn batch_length_is_preserved(count in 1usize..=8) {
        let model = TestPoseModel::echo(model_id(), true);
        let requests: Vec<PoseRequest> = (0..count)
            .map(|i| pose_request(Some(SubjectId::new(i as u32))))
            .collect();
        let outcomes = model.infer(&requests).expect("echo succeeds");
        prop_assert_eq!(outcomes.len(), count);
        for (i, outcome) in outcomes.iter().enumerate() {
            let pose = outcome.pose.as_ref().expect("subject present");
            prop_assert_eq!(pose.subject, SubjectId::new(i as u32));
        }
    }
}
