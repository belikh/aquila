//! The dashboard's wire contract: serde views of the canonical `core-api.md`
//! resources (`docs/spec/core-api.md` §4, §6–§8, §13).
//!
//! These are **read/entry shapes only**. The dashboard is thin over the API
//! (`dashboard.md` §1): it never re-derives domain state, so these structs
//! deserialise what the core serves and serialise the small request bodies the
//! operator submits. Unknown fields are ignored by serde's default, satisfying
//! `core-api.md` §5 ("unknown fields in responses MUST be ignored").
//!
//! The served projections mix naming styles (`/schema` and `/state` are
//! snake_case; `/health`, `/nodes` and `/metrics/snapshot` mix camelCase), so
//! each field carries an explicit `rename` rather than a single
//! `rename_all` — the contract is field-by-field, not convention-by-convention.
//!
//! # Prose
//!
//! Australian English throughout (normalise, serialisation, behaviour).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A list envelope: `{ "items": [ ... ], "next": null }` (`core-api.md` §7).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct List<T> {
    /// The items.
    #[serde(default = "Vec::new")]
    pub items: Vec<T>,
}

/// The home metadata document (`core-api.md` §7).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Home {
    /// `hom_...`.
    #[serde(default)]
    pub id: String,
    /// Human-readable name.
    #[serde(default)]
    pub name: String,
    /// IANA time zone.
    #[serde(default)]
    pub timezone: String,
    /// The number of floors.
    #[serde(default)]
    pub floors: u64,
    /// Concurrency revision.
    #[serde(default)]
    pub revision: i64,
}

/// The home block inside `/schema` (served snake_case).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct SchemaHome {
    /// `hom_...`.
    #[serde(default)]
    pub id: String,
    /// Human-readable name.
    #[serde(default)]
    pub name: String,
    /// IANA time zone (`time_zone` in the served document).
    #[serde(default, rename = "time_zone")]
    pub time_zone: String,
}

/// The zone tree and metric vocabulary served by `GET /schema`.
///
/// The served projection is snake_case (`floor_id`, `elevation_m`), which is
/// what the topology-derived document actually emits (`aquila-api`
/// `projection.rs`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Schema {
    /// The core instance id.
    #[serde(default)]
    pub instance_id: String,
    /// The contract version string.
    #[serde(default)]
    pub contract: String,
    /// The home block.
    #[serde(default)]
    pub home: SchemaHome,
    /// Floors, ordered by level.
    #[serde(default = "Vec::new")]
    pub floors: Vec<Floor>,
    /// Rooms, each carrying its `floor_id`.
    #[serde(default = "Vec::new")]
    pub rooms: Vec<Room>,
    /// The metric vocabulary.
    #[serde(default = "Vec::new")]
    pub metrics: Vec<MetricVocab>,
}

/// A floor (`core-api.md` §7), as served by `/schema`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Floor {
    /// `flr_...`.
    #[serde(default)]
    pub id: String,
    /// Human-readable name.
    #[serde(default)]
    pub name: String,
    /// The floor level; `-1` basement, `0` ground.
    #[serde(default)]
    pub level: i8,
    /// Elevation in metres.
    #[serde(default, rename = "elevation_m")]
    pub elevation_m: f32,
}

/// A room (`core-api.md` §7).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Room {
    /// `rom_...`.
    #[serde(default)]
    pub id: String,
    /// The owning floor (`floor_id` in the served document).
    #[serde(default, rename = "floor_id")]
    pub floor_id: String,
    /// Human-readable name.
    #[serde(default)]
    pub name: String,
    /// The room kind / label.
    #[serde(default)]
    pub kind: String,
}

/// One entry of the metric vocabulary (`GET /schema`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct MetricVocab {
    /// Metric id, lowercase snake_case.
    #[serde(default)]
    pub id: String,
    /// Unit, or `null`.
    #[serde(default)]
    pub unit: Option<String>,
}

/// The coordinator snapshot served by `GET /state` (`aquila-ha`
/// `EntityModel::snapshot`, snake_case).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Snapshot {
    /// The core instance id.
    #[serde(default)]
    pub instance_id: String,
    /// The home id.
    #[serde(default)]
    pub home: String,
    /// Per-zone readings, keyed by room id.
    #[serde(default)]
    pub zones: BTreeMap<String, Zone>,
}

/// One zone's current readings (`GET /state`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Zone {
    /// The zone kind (`"room"`).
    #[serde(default)]
    pub kind: String,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// Presence, or `null` when unknown.
    #[serde(default)]
    pub presence: Option<bool>,
    /// Presence confidence `[0,1]`.
    #[serde(default)]
    pub presence_confidence: Option<f32>,
    /// Person count, or `null`.
    #[serde(default)]
    pub person_count: Option<u16>,
    /// Breathing rate in brpm, or `null`.
    #[serde(default)]
    pub breathing_rate: Option<f32>,
    /// Heart rate in bpm, or `null`.
    #[serde(default)]
    pub heart_rate: Option<f32>,
}

/// Subsystem health (`GET /health`, `core-api.md` §4.1).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Health {
    /// The worst subsystem status: `ok`, `degraded` or `down`.
    #[serde(default)]
    pub status: String,
    /// Uptime in milliseconds.
    #[serde(default, rename = "uptimeMs")]
    pub uptime_ms: u64,
    /// The API version the core advertises.
    #[serde(default)]
    pub api: ApiVersion,
    /// Per-subsystem health.
    #[serde(default = "Vec::new")]
    pub subsystems: Vec<Subsystem>,
}

/// A subsystem health row (`GET /health`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Subsystem {
    /// Subsystem name.
    #[serde(default)]
    pub name: String,
    /// `ok`, `degraded` or `down`.
    #[serde(default)]
    pub status: String,
    /// Optional human detail.
    #[serde(default)]
    pub detail: Option<String>,
}

/// An API version pair (`core-api.md` §5).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct ApiVersion {
    /// Major version.
    #[serde(default)]
    pub major: u16,
    /// Minor version.
    #[serde(default)]
    pub minor: u16,
}

/// The capability document (`GET /capabilities`, `core-api.md` §4.2).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Capabilities {
    /// The API version.
    #[serde(default)]
    pub api: ApiVersion,
    /// Transport availability.
    #[serde(default)]
    pub transports: Transports,
    /// Feature flags.
    #[serde(default)]
    pub features: Features,
    /// The minimum component version.
    #[serde(default, rename = "minComponentVersion")]
    pub min_component_version: String,
}

/// Transport availability (`GET /capabilities`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Transports {
    /// Whether plain HTTP is available.
    #[serde(default)]
    pub http: bool,
    /// Whether the WebSocket stream is available (always `false` in v1).
    #[serde(default)]
    pub websocket: bool,
}

/// Feature flags (`GET /capabilities`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Features {
    /// Polling is the v1 transport.
    #[serde(default)]
    pub polling: bool,
    /// The calibration wizard is reachable.
    #[serde(default, rename = "calibrationWizard")]
    pub calibration_wizard: bool,
    /// Node control commands are reachable.
    #[serde(default, rename = "nodeControl")]
    pub node_control: bool,
    /// Firmware OTA is reachable.
    #[serde(default, rename = "firmwareOta")]
    pub firmware_ota: bool,
}

/// A node (`GET /nodes`, `core-api.md` §7), as served by `aquila-api`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Node {
    /// `nod_...`.
    #[serde(default)]
    pub id: String,
    /// Human-readable name.
    #[serde(default)]
    pub name: String,
    /// The assigned room.
    #[serde(default, rename = "roomId")]
    pub room_id: String,
    /// The factory MAC.
    #[serde(default)]
    pub mac: String,
    /// The hardware type (`esp32s3`, ...).
    #[serde(default)]
    pub hardware: String,
    /// Firmware state.
    #[serde(default)]
    pub firmware: FirmwareInfo,
    /// Node state: `offline`, `online`, `capturing`, `degraded`, `error`,
    /// `unknown`.
    #[serde(default)]
    pub state: String,
}

/// Per-node firmware state (`GET /nodes`, `firmware-ota.md` §7).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct FirmwareInfo {
    /// The node identity the record describes.
    #[serde(default)]
    pub node: String,
    /// The installed version.
    #[serde(default, rename = "installed_version")]
    pub installed_version: String,
    /// The installed image digest, or `null`.
    #[serde(default, rename = "installed_sha256")]
    pub installed_sha256: Option<String>,
    /// The latest available version, or `null`.
    #[serde(default, rename = "latest_version")]
    pub latest_version: Option<String>,
    /// Pending-update flag, or `null`.
    #[serde(default)]
    pub pending: Option<bool>,
}

/// A node-pair (`GET /node-pairs`, `core-api.md` §7).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct NodePair {
    /// `prx_...`.
    #[serde(default)]
    pub id: String,
    /// The transmitting node (`nodeA` in the canonical representation).
    #[serde(default, alias = "nodeA")]
    pub node_a: String,
    /// The receiving node (`nodeB` in the canonical representation).
    #[serde(default, alias = "nodeB")]
    pub node_b: String,
    /// Whether the pair participates in sensing.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// The fusion weight.
    #[serde(default)]
    pub weight: f32,
    /// The core-computed link quality `[0,1]`, or `null`.
    #[serde(default)]
    pub quality: Option<f32>,
    /// `declared` or `derived`.
    #[serde(default)]
    pub source: String,
    /// Concurrency revision.
    #[serde(default)]
    pub revision: i64,
}

/// A metric row (`GET /metrics/snapshot`, `core-api.md` §13).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Metric {
    /// Metric name, e.g. `accuracy.presence_f1`.
    #[serde(default)]
    pub name: String,
    /// `deterministic`, `statistical` or `gated`.
    #[serde(default)]
    pub kind: String,
    /// The unit.
    #[serde(default)]
    pub unit: String,
    /// The value.
    #[serde(default)]
    pub value: f64,
    /// The sample size.
    #[serde(default)]
    pub n: u64,
    /// The reference the value is measured against.
    #[serde(default)]
    pub reference: Option<String>,
    /// An optional pass/fail gate.
    #[serde(default)]
    pub gate: Option<Gate>,
}

/// A metric gate (`core-api.md` §13).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Gate {
    /// The bar the metric must clear.
    #[serde(default)]
    pub bar: f64,
    /// Whether it passed.
    #[serde(default)]
    pub pass: bool,
}

/// A named bar (`GET /metrics/snapshot`, `core-api.md` §13).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Bar {
    /// The target.
    #[serde(default)]
    pub target: f64,
    /// The observed value.
    #[serde(default)]
    pub observed: f64,
    /// Whether it passed.
    #[serde(default)]
    pub pass: bool,
}

/// The commit-keyed metric snapshot (`GET /metrics/snapshot`, `core-api.md`
/// §13). Read-only; keyed on `commit`, never on wall-clock time.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct MetricsSnapshot {
    /// The ledger commit the values were computed at, or `null` when the
    /// ledger is empty.
    #[serde(default)]
    pub commit: Option<String>,
    /// When the snapshot was generated.
    #[serde(default, rename = "generatedAt")]
    pub generated_at: Option<String>,
    /// The snapshot revision.
    #[serde(default)]
    pub revision: u64,
    /// The metric rows.
    #[serde(default = "Vec::new")]
    pub metrics: Vec<Metric>,
    /// The named bars.
    #[serde(default)]
    pub bars: BTreeMap<String, Bar>,
}

/// A calibration session (`core-api.md` §8).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct CalibrationSession {
    /// `cal_...`.
    #[serde(default)]
    pub id: String,
    /// The scope (`global` in v1).
    #[serde(default)]
    pub scope: String,
    /// `pending`, `running`, `awaiting_user`, `computing`, `ready`, `committed`
    /// or `aborted`.
    #[serde(default)]
    pub status: String,
    /// The ordered steps.
    #[serde(default = "Vec::new")]
    pub steps: Vec<CalibrationStep>,
    /// The current step id.
    #[serde(default, rename = "currentStepId")]
    pub current_step_id: Option<String>,
    /// Concurrency revision.
    #[serde(default)]
    pub revision: i64,
}

/// One calibration step (`core-api.md` §8).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct CalibrationStep {
    /// `stp_...`.
    #[serde(default)]
    pub id: String,
    /// The step kind, e.g. `baseline_empty`, `room_tag`.
    #[serde(default)]
    pub kind: String,
    /// The step order.
    #[serde(default)]
    pub order: i32,
    /// The step status.
    #[serde(default)]
    pub status: String,
    /// `user`, `core` or `none`.
    #[serde(default)]
    pub awaiting: String,
    /// The operator-facing instruction.
    #[serde(default)]
    pub instruction: String,
    /// `[0,1]` progress where applicable.
    #[serde(default)]
    pub progress: Option<f32>,
}

/// The request body for `POST /floors`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct NewFloor {
    /// The floor name.
    pub name: String,
    /// The floor index/level.
    pub index: i32,
}

/// The request body for `POST /floors/{floorId}/rooms` and the create-room
/// form (`core-api.md` §6).
#[derive(Debug, Clone, Default, Serialize)]
pub struct NewRoom {
    /// The room name.
    pub name: String,
    /// The room label/kind.
    pub label: String,
}

/// The request body for `PATCH /home`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct HomePatch {
    /// The new name, when changing it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The new time zone, when changing it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

/// The request body for `POST /nodes` (commission).
#[derive(Debug, Clone, Default, Serialize)]
pub struct NewNode {
    /// The node name.
    pub name: String,
    /// The factory MAC.
    pub mac: String,
    /// The assigned room.
    #[serde(rename = "roomId")]
    pub room_id: String,
}

/// The request body for `POST /node-pairs`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct NewPair {
    /// The transmitting node.
    #[serde(rename = "nodeA")]
    pub node_a: String,
    /// The receiving node.
    #[serde(rename = "nodeB")]
    pub node_b: String,
}

/// Serde default for booleans that are enabled unless stated otherwise.
fn default_true() -> bool {
    true
}
