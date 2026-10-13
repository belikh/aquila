# Inference & model pipeline

**Status:** proposed spec. Subordinate to the Aquila brief, `CONTEXT.md`,
ADR-0001 and `docs/spec/architecture.md`. Where this document conflicts with any
of those, they win and the conflict is a defect here.
**Author role:** Inference & model-pipeline author.
**Scope:** the `PoseModel`/`VitalsModel` seam (traits, shapes, device selection,
error modes, fakes), the Python/PyTorch→ONNX pipeline and its provenance, the
Rust inference runtime choice, and loading/warm-up/batching/failure behaviour,
including licence handling.

## 0. Fixed context (restated, not re-litigated)

- Inference runs **in the add-on core**, native Rust, Linux only (x86_64 and
  aarch64 hosts). The dashboard is Rust→WASM served over sidebar ingress; **v1
  does not run models in WASM**.
- Nodes are dumb CSI sensors; they stream `CsiFrame`s over WiFi UDP. The add-on
  does all DSP, inference, fusion, calibration and measurement. Models consume
  **features**, never raw CSI, and never touch the network.
- Vocabulary is `CONTEXT.md`'s and only `CONTEXT.md`'s: `SensingFrame`, `Zone`
  (a `Room` in v1), `Pose`, `Keypoint`, `Vitals`, `Breathing rate`, `Heart rate`,
  `Presence`, `Count`, `Witness`. No synonyms.
- Every sensing claim carries exactly one `Witness`: **Gated**, **Measured**, or
  **Claimed** (strongest first). Grades are derived from the commit-keyed ledger;
  `Claimed` is never surfaced to Home Assistant.
- v1 bars (measurement, not marketing): in-domain torso-PCK@20 ≥ 0.65, presence
  F1 ≥ 0.9, breathing MAE ≤ 2 brpm against a reference. Heart rate has **no v1
  bar** and must be reportable as absent.
- Naming: Rust crates are `aquila-<english-descriptor>`. Prose uses Australian
  English. Code is GPLv3; the project is non-commercial.
- Real seams only, exactly the six in ADR-0001 §10. This document introduces
  **no seventh seam**; model selection is configuration, not a port (ADR-0001
  §6, "Deleted seams").

### 0.1 The L0 contract crate

ADR-0001 names the L0 vocabulary crate **`aquila-types`**; `data-contracts.md`
§0.2 names it **`aquila-contracts`**. This is a genuine cross-doc discrepancy
(flag, §8.1). Below, "the L0 crate" means whichever name is settled; the contract
*types* are exactly those in `data-contracts.md`.

## 1. The `PoseModel` / `VitalsModel` seam

### 1.1 Where it lives

Per ADR-0001 §3 and `architecture.md` §4, the seam is owned by **`aquila-inference`**
(L2). It hides the ONNX runtime, tensor preparation, keypoint post-processing and
model version pinning. The traits speak domain types from the L0 crate; no
`ort`, `ndarray`, `candle` or `burn` type appears in the interface.

| Crate | Layer | Role in this spec |
| --- | --- | --- |
| L0 contract crate | L0 | `SensingFrame`, `FeatureVector`, `Pose`, `Keypoint`, `VitalsReading`, `Detection`, `Witness`, `ContractVersion`, `RoomId`, `NodeId`, `TimeSpan` |
| `aquila-signal` | L1 | produces `FeatureVector` from `SensingFrame`s; the model input |
| `aquila-inference` | L2 | `PoseModel`/`VitalsModel` traits, `OnnxPoseModel`/`OnnxVitalsModel`, `ModelError`, tensor prep, post-processing, model pinning |
| `aquila-testkit` | dev | `FakePoseModel`/`FakeVitalsModel` and fixtures (`testing.md` §5) |
| `aquila-sensing` | L3 | consumes `Arc<dyn PoseModel>`; folds outputs into `Observation`/`Detection`/`Event` |
| `aquila-metrics` / `aquila-evaluation` | L4 / L1 | ledger behind `MetricSink`; `GroundTruth` harness that scores outputs |

Dependency directions are the settled ones: `aquila-inference → L0` and
`aquila-sensing → aquila-inference` (allowed by the DAG; higher depends on
lower). The add-on composition root constructs the concrete ONNX models and
injects them as trait objects; nothing above L2 names `ort`.

`ModelError` lives in `aquila-inference` (it is the seam's error). The value
types it carries live in the L0 crate.

### 1.2 Contract types (consumed, not redefined)

The seam consumes and produces the types fixed in `data-contracts.md`; this
document does **not** redefine them. Summarised for orientation:

- **Input:** one `FeatureVector` per `SensingFrame`, plus the scope it belongs to.
  `SensingFrame { scope: SensingScope, span: TimeSpan, grid: ClockGrid, frames: Vec<CsiEnvelope> }`;
  `SensingScope = Node(NodeId) | Pair(PairId)`.
- **Pose output:** `Pose { subject, room: RoomId, t_unix_ms, keys: [Keypoint; 17], score, witness }`,
  COCO-17 order, room-local metres, `z_m: Option<f32>` (planar in v1). A missing
  keypoint is `confidence == 0.0` with `x_m`/`y_m` possibly `NaN` — never invented.
- **Vitals output:** `VitalsReading { subject: Option<SubjectId>, room, span, breathing: Option<Estimate>, heart_rate: Option<Estimate>, status: VitalsStatus, witness }`.
  `status != Ok` requires the affected estimate to be `None`; a reading never
  claims "no signal" and reports a value simultaneously.
- **Presence / Count:** `Detection { scope: DetectionScope, span, kind: DetectionKind, confidence, value: DetectionValue, contributors: Vec<NodeId>, witness }`,
  with `kind` `Presence`/`Count` and matching `value` variant.
- **Witness:** `Gated | Measured | Claimed`, derived from ledger records.
- **Version:** `ContractVersion { major, minor }`; the runtime and the model card
  both pin it.

### 1.3 Trait interfaces

The architecture gives each seam "one method each". Batching is expressed by a
slice-in, `Vec`-out method so the dynamic `B` axis is real, while remaining one
method per trait.

```rust
// aquila-inference
use aquila_types::{
    Detection, DetectionScope, FeatureVector, NodeId, Pose, RoomId, SubjectId,
    TimeSpan, VitalsReading,
};

/// One unit of pose work: a feature set, the scope it came from, and the
/// topology facts the head needs to emit a contract-shaped result.
pub struct PoseRequest {
    pub scope: DetectionScope,       // Room(zone) or Home; v1: Room
    pub contributor_nodes: Vec<NodeId>, // non-empty, ordered; copied into Detections
    pub subject: Option<SubjectId>,  // core-assigned subject slot, if any
    pub span: TimeSpan,
    pub features: FeatureVector,     // from aquila-signal (layout owned there)
}

/// Pose plus the shared-trunk auxiliary heads. One inference call, one bundle.
pub struct PoseOutcome {
    pub pose: Option<Pose>,          // None ⇒ no pose claim this span (honest)
    pub presence: Option<Detection>, // DetectionKind::Presence
    pub count: Option<Detection>,    // DetectionKind::Count
}

pub trait PoseModel: Send + Sync {
    fn id(&self) -> &ModelId;
    /// `requests` is non-empty; returns one outcome per request, same order.
    fn infer(&self, requests: &[PoseRequest]) -> Result<Vec<PoseOutcome>, ModelError>;
}

pub struct VitalsRequest {
    pub scope: DetectionScope,
    pub contributor_nodes: Vec<NodeId>,
    pub subject: Option<SubjectId>,
    pub span: TimeSpan,
    pub features: FeatureVector,
}

pub trait VitalsModel: Send + Sync {
    fn id(&self) -> &ModelId;
    /// `requests` is non-empty; returns one reading per request, same order.
    fn infer(&self, requests: &[VitalsRequest]) -> Result<Vec<VitalsReading>, ModelError>;
}
```

Notes:

- `id()` returns the content-addressed `ModelId` (§2.5); it is echoed into the
  ledger so a metric is always tethered to the exact weights that produced it.
- `Witness` on model-emitted results is `Measured` (it ran and is recorded).
  `Gated` is assigned later by the ledger (§5); `Claimed` is only ever produced
  by a fake and is filtered at the `Sink`.
- Presence and Count ride on the pose trunk because the brief fixes the seam set
  and the architecture assigns them to the same features. This is the one place
  this spec extends the `data-contracts.md` §1 seam-mapping line, which lists
  `Pose`/`VitalsReading` only (flag, §8.2).

### 1.4 Feature and tensor shapes

The `FeatureVector` layout is owned by `aquila-signal` (`architecture.md` §3,
`signal.md`); the ONNX tensor mapping is **internal to `aquila-inference`**. Both
are pinned in the model card. Named dimensions:

| Symbol | Meaning | v1 default |
| --- | --- | --- |
| `B` | requests in a batch (dynamic in the graph) | 1..=8 |
| `T` | `SensingFrame` slots per pose span | 100 (1 s at 100 Hz) |
| `F` | spectrogram bins in the vitals band | 128 |
| `S` | subcarriers | 64 @ 20 MHz HT, 128 @ 40 MHz |
| `L` | contributing links (RX×TX), pair/zone total | ≤ 3 per node-pair, ≤ 24 per zone |
| `K` | keypoints | 17 (COCO-17, fixed) |
| `C_MAX` | persons scored by the count head | 8 (contract cap) |

Internal ONNX I/O, fixed at export with `B` the only dynamic axis:

- Pose graph inputs: `amplitude [B,T,S,L] f32`, `phase [B,T,S,L] f32`,
  `link_mask [B,L] bool`; outputs `keypoints [B,K,3] f32`,
  `keypoint_confidence [B,K] f32`, `presence_logit [B] f32`,
  `count_logits [B,C_MAX+1] f32`.
- Vitals graph inputs: `phase_spectrogram [B,F,S,L] f32`,
  `amplitude_spectrogram [B,F,S,L] f32`, `link_mask [B,L] bool`; outputs
  `breathing_bpm [B] f32`, `heart_bpm [B] f32`, `breathing_ok [B] bool`,
  `heart_ok [B] bool`.

`aquila-inference` is responsible for turning each `FeatureVector` into exactly
`T`,`S`,`L` (pose) or `F`,`S`,`L` (vitals) as the card declares. Anything else is
`UnsupportedInputShape`, never a silent resize or pad. Absent links are explicit:
`link_mask` is authoritative, zero-padding is permitted only where the mask says
absent. The exact `FeatureVector`→tensor projection must be frozen with
`aquila-signal` before the ONNX adapter is implemented (§8.3).

Post-processing (inside `aquila-inference`): map `[B,K,3]` to `[Keypoint; 17]` in
COCO-17 order, clamp `confidence`/`score` to `[0,1]`, set `NaN` where the model
gives no estimate, thin presence `presence_logit`→`P(occupied)` with a configured
threshold, and `argmax` the count logits to `DetectionValue::Count`. No smoothing
or hysteresis here — that is `aquila-sensing`'s job.

### 1.5 Device selection & determinism

Device selection is **construction configuration**, not a seam parameter and not
a port (ADR-0001 §6 deletes `ModelRegistry`; model selection is configuration).

```rust
#[derive(Clone, Debug)]
pub enum DeviceSelection {
    Cpu { intra_threads: usize, inter_threads: usize }, // v1 default
    Cuda { device_id: i32 },          // #[cfg(feature = "cuda")]
    OpenVino { device: String },      // #[cfg(feature = "openvino")]
}
```

- Default and v1 target: `Cpu { intra_threads: 4, inter_threads: 1 }`, the
  effective value clamped to `available_parallelism()` and **recorded** in the
  ledger key, so latency and results reproduce on a given host.
- `Cuda`/`OpenVino` are feature-gated and not required. HA hosts are commonly
  fanless x86 mini-PCs or aarch64 SBCs; CPU is the honest assumption.
- **Determinism is scoped, not assumed.** ONNX Runtime floating point is not
  guaranteed bit-identical across execution providers, thread counts or library
  versions. Ledger comparisons key on
  `(model_digest, ep, intra_threads, inter_threads, ort_version)` and diff only
  within a fixed key. Cross-key agreement is a comparison, never an equality
  claim. This matches `testing.md`'s "bit-exact on the canonical target" tier.
- The exported graph must not require any CPU-unsupported op; the parity gate
  (§2.4) is where that fails.

### 1.6 Error modes (typed; never fabricate)

```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ModelError {
    #[error("model not loaded: {0}")]                              NotLoaded(ModelId),
    #[error("model not warmed up: {0}")]                           NotWarmedUp(ModelId),
    #[error("model digest mismatch: {model}")]                     DigestMismatch { model: ModelId },
    #[error("contract version mismatch: want {want:?}, got {got:?}")] ContractMismatch { want: ContractVersion, got: ContractVersion },
    #[error("licence/witness declaration missing or conflicting")] LicenceMismatch { detail: String },
    #[error("empty request batch")]                                EmptyBatch,
    #[error("unsupported input shape for {name}: expected {expected:?}, got {got:?}")]
    UnsupportedInputShape { name: &'static str, expected: Vec<Dim>, got: Vec<usize> },
    #[error("dtype mismatch for {name}: expected {expected:?}, got {got:?}")]
    DtypeMismatch { name: &'static str, expected: DType, got: DType },
    #[error("feature vector malformed: {0}")]                      FeatureMalformed(String),
    #[error("device unavailable: {device:?} ({detail})")]          DeviceUnavailable { device: DeviceSelection, detail: String },
    #[error("engine build failed: {0}")]                           EngineBuild(String),
    #[error("inference failed at {stage}: {detail}")]              InferenceFailed { stage: String, detail: String },
    #[error("output shape mismatch for {name}: expected {expected:?}, got {got:?}")]
    OutputShapeMismatch { name: &'static str, expected: Vec<Dim>, got: Vec<usize> },
    #[error("non-finite output for {name} at {index}")]            NumericFault { name: &'static str, index: usize },
    #[error("inference timeout at {stage} after {ms} ms")]         Timeout { stage: String, ms: u64 },
    #[error("model unavailable: {0:?}")]                           Unavailable(ModelHealth),
    #[error("internal error: {0}")]                                Internal(String),
}
```

Rules:

1. Every failure returns `Err`. There is **no** default, zeroed, last-known-good
   or "best guess" output. A failed request produces no `Pose`, `VitalsReading` or
   `Detection` for that span.
2. `NumericFault` is raised if any output element is NaN/Inf; the batch is dropped
   and counted. Input NaN is permitted only where `signal.md` defines it (a masked
   link); output NaN is never coerced into a claim.
3. Missing estimates use the contract's own representation, not an error: a
   pose with no estimate sets `confidence = 0.0` (`x_m`/`y_m` may be `NaN`); a
   vitals reading with no estimate sets the `Option` to `None` with a `VitalsStatus`.
   These are honest *outputs*; `ModelError` is for *failure to run*.
4. Errors carry structure for the `MetricSink`/ledger and for the health surface
   on `aquila-api`'s private HTTP+WS control plane. A model that failed warm-up
   reports `Unavailable`, and `infer` returns `ModelError::Unavailable`.

`ModelHealth` is `Warming | Warm | Degraded { reason } | Unavailable { error }`.

### 1.7 Fake / test adapters

Fakes are adapters, not mocks, and live in **`aquila-testkit`** (`testing.md` §5),
gated out of release builds. They implement the *same* traits as the real models.

| Double | Modes (constructed, not a new trait) |
| --- | --- |
| `FakePoseModel` | `Identity`, `Constant`, `Table` (keyed by input digest), `Scripted` (per-call `Result`), `Failing(ModelError)`, `SeededRng(seed)` |
| `FakeVitalsModel` | same, plus forced `breathing: None` / `heart_rate: None` and forced `VitalsStatus` |

Requirements:

- Deterministic: same input ⇒ same output, including `SeededRng`.
- Never reachable from production wiring: the composition root accepts only
  `aquila-inference` types; `aquila-testkit` is a dev-dependency.
- Inputs validated with the same `TensorSpec` logic as real models, so a shape
  bug fails identically in tests and production.
- `Table`/`Scripted` replay committed golden outputs for pipeline, calibration
  and fusion tests, which never depend on ONNX.

Accuracy tests do **not** use fakes; they run the real ONNX graph over fixtures
under a pinned `(ep, threads, ort_version)` key and record to the ledger
(`testing.md` tier 4; `aquila-evaluation`).

## 2. Model pipeline: Python/PyTorch → ONNX

### 2.1 Layout and flow

```
ml/                       # Python; not a crate; not shipped to the HA host
├── aquila_ml/
│   ├── data/             # MM-Fi loader, own Sessions, replay fixtures
│   ├── seed/             # ruvnet NC weight import
│   ├── models/           # trunk + pose/vitals/presence/count heads (PyTorch)
│   ├── train.py
│   ├── export_onnx.py
│   └── parity.py         # PyTorch↔ONNX numerical gate
├── configs/              # YAML: dataset, shapes, epochs, seeds, opset
└── fixtures/             # golden input/output pairs
models/                   # content-addressed artefact store (see below)
config/model-manifest.json  # name@version → path; configuration, not a seam
```

Flow: **seed → fine-tune → export → parity gate → model card → store admission.**

### 2.2 Seeding from the ruvnet NC weights

- The ruvnet NC weights initialise the shared trunk (transfer learning). The
  import records their exact SHA-256, upstream reference and licence string; it
  never drops this provenance.
- The model card's `weights_trained` distinguishes provenance states:
  `Imported` (ruvnet seed, untrained by us — `Witness::Claimed` until measured),
  `FineTunedFromSeed` (seed + our data), `TrainedFromScratch` (no seed).
  "Imported" alone is never presented as a working Aquila model.
- Heads the seed does not provide are initialised by config and documented in the
  card, not guessed.
- Seeding is configuration, not a hard dependency: `TrainedFromScratch` must stay
  possible so the pipeline can show the seed actually helps.

### 2.3 Datasets

| Data | Use | Licence posture | Notes |
| --- | --- | --- | --- |
| ruvnet NC weights | trunk seed | NC — recorded verbatim | accepted: Aquila is non-commercial (§6) |
| MM-Fi | pose fine-tune / evaluation | verify and record exact upstream licence string | treat as NC/research unless the string says otherwise |
| Own recorded `Session`s | fine-tune / evaluation | Aquila-controlled | CSI from reference ESP32-S3 nodes |
| Camera ground truth | **research-only** labels for own Sessions | Aquila-controlled; never shipped, never a runtime input | `CONTEXT.md`: research-only |
| Replay / simulator fixtures | tests, out-of-CI metrics | Aquila-controlled | determinism and regression |

Each dataset entry records name, version or split digest, shape conventions, the
exact licence string, and the `Witness` it can support. A model may not claim a
grade its data cannot support.

### 2.4 Training and export

- PyTorch, config-driven, fixed seeds, deterministic dataloaders where possible.
- Losses: pose = keypoint regression plus per-keypoint confidence BCE;
  presence = BCE; count = cross-entropy over `0..=C_MAX`; vitals = masked MAE on
  breathing (heart loss active only where the reference is trustworthy).
- Export: `torch.onnx.export`, pinned opset (default 17), dynamic `batch` axis
  only, input/output names exactly as §1.4. Names are part of the contract.
- **Parity gate (mandatory):** run the exported graph over held-out inputs and
  assert max abs/rel deviation from PyTorch within a configured tolerance under
  the CPU EP. Failure ⇒ no store admission. This gate surfaces CPU-op
  unsupportedness and numeric divergence before anything ships.
- Export writes the `.onnx` and the model card into the store (§2.5) in one step.

### 2.5 Versioning and the model store (configuration, not a seam)

ADR-0001 §6 deletes `ModelRegistry` as a hypothetical seam: "model selection is
configuration (a weights path), not a port." So there is **no registry port**.

- **Content addressing.** `ModelId` = lowercase hex SHA-256 of the `.onnx` bytes.
  The card is separately digested.
- **Store.** A content-addressed directory referenced by
  `config/model-manifest.json`, in the same spirit as `config/topology.json`
  (`data-contracts.md` §3). The manifest pins `name@version → {onnx_path,
  card_path, digests}`; the add-on loads from it. No network fetch: the brief
  forbids a public surface, and model download would be new egress.
- Semantic versioning on the model, independent of the code version; the card
  pins the **code commit**, config digest and `ContractVersion` used.
- The runtime asserts `card.trained_contract_version == runtime CONTRACT_VERSION`
  and that declared input/output specs match the expected contract; mismatch ⇒
  `ContractMismatch`, refuse. Upgrading the shipped model is an explicit manifest
  change, never a floating tag.

### 2.6 Provenance and model card

`model_card.json` is required for admission. Minimum fields:

| Field | Type | Purpose |
| --- | --- | --- |
| `name`, `semver` | string | identity |
| `onnx_sha256` | hex | content address; must equal the file digest |
| `task` | `pose` \| `vitals` | which trait it serves |
| `trained_contract_version` | `ContractVersion` | matches the runtime |
| `weights_trained` | `Imported` \| `FineTunedFromSeed` \| `TrainedFromScratch` | provenance honesty |
| `seed_weights` | object \| null | `{ref, sha256, licence}` for the ruvnet seed |
| `datasets` | array | `{name, version, split, licence, sha256, witness}` each |
| `licence` | string | the **weights** licence (may be NC) |
| `commercial_use` | bool | `false` for v1 |
| `witness` | `Witness` | strongest grade the evidence supports, derived per §0 |
| `input_spec`, `output_spec` | spec arrays | fixed dims, dtype, names |
| `opset`, `dynamic_axes` | int / map | export facts |
| `feature_map` | object | exact `FeatureVector`→tensor projection, with digest |
| `eval` | array | `{metric, value, witness, ledger_key, dataset, n}` |
| `training` | object | code commit, config digest, hardware, date, epochs, seed |

`witness` is derived, never asserted: `Gated` iff a `MetricKind::Gated` ledger
record with `passed == true` backs a claimed bar; `Measured` iff any non-gated
record backs it; `Claimed` otherwise (`data-contracts.md` §1). A model whose
card claims a v1 bar must cite a passing `Gated` record, or the card is rejected.

## 3. Rust inference runtime: options and recommendation

### 3.1 Options

| Option | Shape | Strengths | Weaknesses for Aquila |
| --- | --- | --- | --- |
| **`ort`** (ONNX Runtime bindings) | FFI to the ONNX Runtime C/C++ library | Full ONNX operator coverage; executes the graph as exported, so PyTorch→ONNX parity holds by construction; CPU EP on x86_64 and aarch64 Linux; optional CUDA/OpenVINO EPs; mature dynamic shapes; MIT/Apache-2.0, GPLv3-compatible | Native dependency (bundled or system `libonnxruntime`); FFI surface; binary size; ONNX Runtime version must be pinned for reproducibility |
| **`candle`** (Hugging Face) | Pure-Rust tensor/ML library | No C/C++ dependency; strong cross-compile and WASM targets; clean API | `candle-onnx` supports only a **subset** of ONNX; re-implemented kernels risk numeric divergence from the PyTorch export; ONNX import is not its first-class goal, so arbitrary exported graphs may need hand-porting |
| **`burn`** | Pure-Rust DL framework (own IR, multiple backends) | Rust-native, trainable in Rust, good WASM/embedded story, pluggable backends | Requires re-authoring the model in Burn's API or accepting `burn-import` ONNX limitations; op coverage and parity are not guaranteed for an arbitrary export; as a framework it invites re-doing training in Rust, contradicting the settled PyTorch pipeline |

### 3.2 Recommendation: `ort` (ONNX Runtime), CPU execution provider, pinned

Adopt **`ort`** for v1 inference.

1. **The pipeline is PyTorch→ONNX.** `ort` executes the exported graph, so the
   parity gate guarantees shipped numerics match training. `candle` and `burn`
   re-implement or partially import the graph, reintroducing parity risk exactly
   where the brief wants it eliminated.
2. **Operator coverage.** Pose/vitals graphs contain ordinary conv/attention/LSTM
   blocks; ONNX Runtime covers them fully. `candle-onnx` does not guarantee so.
3. **Host reality.** HA add-ons run on CPU. ONNX Runtime ships official CPU
   builds for x86_64 and aarch64 Linux; GPU EPs are available later behind a
   feature flag without changing the seam.
4. **Licence.** ONNX Runtime is MIT/Apache-2.0 — GPLv3-compatible.
5. **The seam isolates the choice.** Because `aquila-inference` hides the engine
   and `aquila-sensing` sees only trait objects, a future niche (e.g. a tiny
   model in the WASM dashboard) can use `candle` without touching the seam.

Concretely: pin the ONNX Runtime and `ort` versions together and record both in
the ledger key; prefer a **statically bundled** runtime for reproducibility;
default `DeviceSelection::Cpu { intra_threads: 4, inter_threads: 1 }`;
feature-gate `cuda`/`openvino` off by default. Do not adopt `candle` or `burn`
for v1. This choice deserves an ADR; this document does not create one.

## 4. Loading, warm-up, batching, failure behaviour

### 4.1 Loading

1. Resolve `name@version` from `config/model-manifest.json` to store paths;
   **no network**.
2. Read `.onnx` bytes; SHA-256; compare to `card.onnx_sha256`. Mismatch ⇒
   `DigestMismatch`, refuse.
3. Parse the card; require `licence`, `commercial_use`, `witness` present.
   Missing/conflicting ⇒ `LicenceMismatch`.
4. Assert `card.trained_contract_version == CONTRACT_VERSION` and that declared
   input/output specs match the trait's expected contract. Mismatch ⇒
   `ContractMismatch`, refuse.
5. Build the ONNX Runtime engine with the selected `DeviceSelection`; failure ⇒
   `EngineBuild`.
6. Warm up (§4.2). Only a `Warm` model serves.

Loading is eager at add-on start (health known immediately) and non-fatal: a
model that fails to load does not abort unrelated subsystems; it reports
`Unavailable` and its epochs are absent, never fabricated.

### 4.2 Warm-up

- Run a fixed warm-up set: `B = 1` and `B = max_batch_size` canonical batches
  from a committed representative fixture (not noise), for a configured number of
  iterations.
- Warm-up allocates arenas and thread-pool state; outputs are discarded, not
  emitted and not recorded as predictions.
- Warm-up failure ⇒ `Unavailable`; the model never serves.
- Warm-up timing is recorded so the ledger separates cold from warm comparisons.

### 4.3 Batching

- The seam takes a `&[PoseRequest]` / `&[VitalsRequest]` slice. `aquila-sensing`
  forms batches; `OnnxPoseModel`/`OnnxVitalsModel` accept dynamic `B`.
- `max_batch_size` (default 8) and `batch_timeout_ms` (default 20 ms) live in
  config. Requests for a zone are grouped; v1 keeps batching per zone for
  simplicity, though the graph is zone-agnostic.
- **Backpressure is honest:** the queue is bounded; overflow drops the **oldest**
  request and emits a `Sink` drop event with counts. Dropped requests produce no
  output and are never back-filled.
- Batch assembly validates every row against the model spec before inference; one
  malformed row fails that batch with a typed error rather than corrupting the
  others.

### 4.4 Failure behaviour — never fabricate

1. Any `ModelError` ⇒ no output for that batch. No zeros, no last-known-good, no
   interpolation, no "confidence 0" fabrication.
2. Non-finite outputs ⇒ `NumericFault` ⇒ batch dropped and counted.
3. Repeated failures flip health to `Degraded`/`Unavailable` and surface over
   `aquila-api`'s **private** HTTP+WS control plane to Home Assistant.
4. Every served, dropped and failed span is accounted to `MetricSink`/ledger with
   the ledger key, so "did the model actually run?" is measurable.
5. The card's `witness` and metrics travel with the model so no consumer can show
   a number untethered from its evidence.

## 5. Measurement coupling (record + diff)

- Rust-side `FakePoseModel`/`FakeVitalsModel` `Table` mode replays committed
  golden outputs keyed by input digest; deterministic metrics are computed from
  these under a fixed ledger key.
- The Python side emits the same fixtures and PyTorch reference outputs; the
  parity gate (§2.4) compares them.
- Everything is keyed by **code commit**, per the brief's commit-keyed ledger,
  which `aquila-metrics` owns behind `MetricSink` (`LedgerMetricSink` for the
  persistent ledger, `InMemoryMetricSink`/`RecordingMetricSink` for tests).
- This matches `measurement.md`: `MetricKind` is deterministic/statistical/gated;
  no CI gating in v1 — the ledger records and diffs.
- Gated metrics (presence F1, breathing MAE, torso-PCK@20) are computed only where
  `Witness::Gated` evidence exists; otherwise reported as absent, never as
  passing. `aquila-evaluation`'s `Harness` aligns predictions to `GroundTruth`
  in time and scope before scoring.

## 6. Licensing

Aquila is GPLv3 and **non-commercial**; that is settled and answers the NC
question: NC datasets and NC weights are **acceptable** for this project.

Nuances that must be honoured:

1. **Code and weights are licensed separately.** GPLv3 covers the source. The
   ruvnet NC weights, MM-Fi, and any weights derived from them carry their own NC
   terms. NC is a *use restriction*, not a software copyleft; it neither
   relicenses the GPLv3 code nor is laundered off the weights by it.
2. **Derivative weights stay NC.** A model seeded from ruvnet NC weights and/or
   fine-tuned on MM-Fi is a derivative and inherits NC: it cannot be offered under
   GPLv3 alone and cannot be used commercially.
3. **Keep NC artefacts out of the GPLv3 history.** Dataset bytes and NC weight
   files live in a separate artefact store, not committed to git. The repo
   commits code, configs, small owned fixtures, and model cards (which *record*
   NC provenance). NC-derived `.onnx` payloads ship under a `MODEL-LICENCE` file
   stating NC, alongside the GPLv3 code.
4. **The card carries the truth.** Every card has `licence` (weights) and
   `commercial_use: false`; the loader refuses a card missing either
   (`LicenceMismatch`). A future commercial build would have to re-derive from
   permissively licensed data/weights, and the card is what makes that auditable.
5. **Runtime dependencies.** ONNX Runtime (`ort`) is MIT/Apache-2.0, compatible
   with GPLv3. PyTorch, NumPy and the training stack are permissive and are not
   shipped to the HA host.
6. **Own Sessions and camera ground truth.** Own CSI Sessions are ours to
   licence. Camera-derived ground truth is research-only per `CONTEXT.md`: not
   shipped, not a runtime input, not needed by any runtime artefact.
7. **Record exact strings.** Every dataset/weight licence is recorded verbatim in
   the card; "NC" as a hand-wave is not provenance.

## 7. Non-goals

- No model download over the network; no public inference API, REST, WS or MCP.
- No WASM inference in v1; the dashboard displays results, it does not compute them.
- No in-Rust training; no `burn`/`candle` training path.
- No PoE, no macOS, no non-Linux host.
- No phase-coherent RF reference; feature-level fusion only.
- No camera or ESP32-as-SDR path at runtime; both are research-only.
- No CI gating of metrics in v1 (record + diff only).
- No new seams: presence and count ride on `PoseModel`; model selection is config.

## 8. Cross-doc alignment and open questions

### 8.1 Flags

1. **L0 crate name.** ADR-0001 says `aquila-types`; `data-contracts.md` §0.2 says
   `aquila-contracts`. Resolve before any crate is created (conflicts with
   ADR-0001 as written).
2. **Seam output shape.** `data-contracts.md` §1 maps `PoseModel`/`VitalsModel` →
   `Pose`/`VitalsReading`. This spec returns `PoseOutcome` (pose + presence/count
   heads) so the shared trunk is one call. Recommend `data-contracts.md` adopt
   `PoseOutcome` or split the heads; either is acceptable, but the two documents
   must agree.
3. **`FeatureVector` layout.** Not yet pinned in `data-contracts.md`. `signal.md`
   owns it. The `FeatureVector`→tensor projection and its digest are required
   before the ONNX adapter is implemented.
4. **`aquila-testkit`.** `testing.md` §5 places the fakes there; ADR-0001 does not
   list it. Confirm it is a dev-only crate outside the shipped DAG (it appears to
   be), or fold the fakes into each seam crate behind a `testing` feature.

### 8.2 Open questions (to grill, not to guess)

1. Exact ruvnet NC weight format, layer coverage and licence string — capture
   before specifying the seed import further.
2. MM-Fi licence string (upstream terms vary by release) — record verbatim.
3. `T`, `S`, `L`, `F` defaults depend on firmware CSI rate and channel width;
   confirm against the ESP32-S3 capture path (`capture-and-firmware.md`) before
   freezing.
4. Presence/count head placement (pose trunk vs `aquila-sensing`) — settle with
   the contracts owner (§8.1.2).
5. Whether multi-zone batches (zone-agnostic trunk) are worth it later.

## 9. Summary (as requested)

The `PoseModel`/`VitalsModel` seam lives in `aquila-inference` (L2), speaking the
L0 contract types — `FeatureVector` in, `Pose`/`VitalsReading` and presence/count
`Detection`s out — so no engine type leaks. Two object-safe traits each expose one
batch-slice method; ONNX `[B,T,S,L]`/`[B,F,S,L]` shapes, device selection and
post-processing stay inside. A typed `ModelError` guarantees no fabricated output;
absence is expressed through the contract (zero confidence, `None` estimates).
`aquila-testkit` supplies deterministic `FakePoseModel`/`FakeVitalsModel`.
Training is Python/PyTorch, seeded from the ruvnet NC weights, fine-tuned on MM-Fi
plus own Sessions, exported to ONNX under a mandatory PyTorch↔ONNX parity gate,
and admitted to a content-addressed store (configuration, not a seam) only with a
card carrying `weights_trained`, exact datasets, verbatim licence strings and a
ledger-derived `Witness`. NC data and weights are acceptable because Aquila is
non-commercial; derivative weights stay NC and out of the GPLv3 history. The
runtime loads locally, verifies digest and `ContractVersion`, warms up, batches
with honest backpressure, and surfaces typed health over `aquila-api`.

**Runtime recommendation:** use **`ort` (ONNX Runtime), CPU execution provider,
version-pinned and statically bundled**. It executes the exported graph unchanged,
so the parity gate holds; `candle` and `burn` only partially import ONNX and would
reintroduce numeric divergence. The seam's domain-typed interface keeps a future
`candle` swap possible without touching the core.
