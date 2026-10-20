# Aquila data contracts (canonical, versioned)

Status: settled. This document is the **single source of truth** for Aquila's
data types. Every concept below is defined **once**. No other crate, firmware,
model, session or fixture may define a second, parallel version of these types.

This file exists because RuView carried eight different `CsiFrame` definitions and
thirteen `Args` types. Aquila does not repeat that. If you need a shape that is
*almost* `CsiFrame`, add a field or wrap it — do not fork it.

Terminology follows the repository's canonical vocabulary in `CONTEXT.md`. Where
this document and `CONTEXT.md` name the same thing, `CONTEXT.md` wins on *naming*
and this document supplies the *shape*.

- Contract version: **`1.0`** (`CONTRACT_VERSION`).
- Wire version: **`1.0`** (`WIRE_MAJOR` / `WIRE_MINOR`), carried in every UDP frame.
- English: Australian spelling throughout (serialisation, behaviour, metres,
  normalise, synchronise, calibration, licence).

---

## 0. Two versions, and the one rule

There are exactly **two version numbers**, and they move together:

| Version | Carried by | Scope |
| --- | --- | --- |
| **Contract version** | `ContractVersion` on every `Session`, `LedgerRecord`, session archive | The set of serde types in `aquila-types` |
| **Wire version** | Bytes 4–5 of every UDP header | The on-air binary layout only |

The **one rule**: *a contract is append-only within a major version.* You may add
optional, defaulted fields and new explicitly-fallback enum variants. You may
never remove, reorder, retype, re-unit or semantically repurpose an existing
field or variant. Anything else is a **major** bump.

### 0.1 Major vs minor

- **MINOR (`1.0` → `1.1`)**: additive only. New optional field with
  `#[serde(default)]`; new enum variant **only** where receivers already handle an
  `Unknown` fallback (see §0.3). Old readers ignore what they don't know.
- **MAJOR (`1.x` → `2.0`)**: any removal, retype, unit change, meaning change,
  reordering, or change to an invariant. Requires: this document revised, the
  `aquila-types` crate major-bumped, the wire header major-bumped, and a
  migration note. A core receiving a mismatched **wire major** MUST reject the
  datagram and record it once per `(node_id, boot_epoch)`. It MUST NOT guess.
- **Units and semantics are part of the contract.** Changing `f32` metres to
  `f32` millimetres is a MAJOR change even though the Rust type is identical.
- **Deprecate before removing.** A field is marked deprecated here for at least
  one minor release, then removed at the next major. Never repurpose a name.

### 0.2 Single crate, no forks

All types live in **`aquila-types`** — the L0 vocabulary crate (ADR-0003 §2).
Every other crate — `aquila-wire`, `aquila-signal`, `aquila-geometry`,
`aquila-metrics`, `aquila-session`, `aquila-inference`, `aquila-sources`,
`aquila-sensing` and the L4 edges — imports them. Adapters may *wrap* a contract
type (e.g. `CsiEnvelope`), never re-declare it. A repository check asserts that
the canonical struct names are declared in exactly one crate.

### 0.3 Unknown values are preserved, never invented

Enums that can gain variants (`EventKind`, `VitalsStatus`, `Band`, `RoomKind`)
each carry an explicit `Unknown`/`#[serde(other)]` arm. A reader that meets an
unrecognised variant keeps the raw discriminant where the format allows and
otherwise maps it to `Unknown`. It never reinterprets an unknown variant as a
known one.

---

## 1. Shared primitives

```rust
use serde::{Deserialize, Serialize};

/// The contract-set version. Semver-with-major-minor only; no patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractVersion { pub major: u16, pub minor: u16 }
pub const CONTRACT_VERSION: ContractVersion = ContractVersion { major: 1, minor: 0 };

/// Dense on-air identity, unique within one Home. Provisioned via the node
/// control channel at commissioning and mirrored in Topology. 1..=65535;
/// 0 is reserved for "unprovisioned".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u16);
impl NodeId { pub const UNPROVISIONED: NodeId = NodeId(0); }

/// Stable, human-authorable identity for config/topology entities.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Slug(pub String);           // e.g. "kitchen-01", "ground-floor"

pub type HomeId    = Slug;
pub type FloorId   = Slug;
pub type RoomId    = Slug;
/// Identity of a virtual detection region. A zone may be coextensive with a
/// room, a sub-region of a room (for example "bed" inside the bedroom), or span
/// multiple rooms (`Zone.room` is `None` when it spans rooms).
pub type ZoneId    = Slug;
pub type PairId    = Slug;
pub type SessionId = Slug;
pub type MetricId  = Slug;

/// 48-bit hardware address. Lower-case hex on the wire and on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MacAddr(pub [u8; 6]);

/// Session-scoped anonymous subject index. Stable only within a Session; it is
/// not person identity and is never persisted across sessions as identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SubjectId(pub u32);

/// Half-open time span on the host monotonic clock, then wall time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimeSpan {
    pub start_mono_ns: u64,
    pub end_mono_ns: u64,          // invariant: > start_mono_ns
    pub start_unix_ms: i64,
    pub end_unix_ms: i64,
}

/// 2.4 / 5 GHz. `Unknown` is a first-class value, not an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Band { B2G4 = 0, B5G = 1, #[serde(other)] Unknown = 255 }

/// Striped OFDM secondary channel (ESP rx_ctrl). `None` = 20 MHz primary only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecondaryChannel { None = 0, Above, Below, #[serde(other)] Unknown = 255 }
```

**`Witness`** — the evidence grade every sensing claim carries, exactly one. This
is the `CONTEXT.md` vocabulary; do not invent synonyms (no "validated",
"verified", "proven", "trusted"). There are exactly **five** grades, ordered
strongest first; this is the *single* authoritative set (there is no three-grade
set — `evaluation.md` §5 refines the same five and the implementation
`aquila_types::Witness` declares them):

| # | Grade | Meaning | May carry a number |
| --- | --- | --- | --- |
| 0 | `Gated` | Measured **and** passing a defined v1 bar. The only grade a headline claim may carry. Derived from a `Gated` `LedgerRecord` with `passed == true`. | Yes |
| 1 | `Measured` | Computed from real, replayed or simulated input and recorded in the commit-keyed ledger. True by construction for any recorded result. | Yes |
| 2 | `Claimed` | Asserted without a recorded measurement. Never emitted to Home Assistant. | No |
| 3 | `DataGated` | Blocked on missing input: no fixture, no labels, no reference recording, or insufficient paired samples. A named, pending measurement. A blocked sub-grade of `Claimed`. | No |
| 4 | `HardwareGated` | Blocked on missing hardware: a reference device, chest strap or belt, camera rig, or the reference-scale node deployment. A named, pending measurement. A blocked sub-grade of `Claimed`. | No |

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Witness {
    /// Measured and passing a defined v1 bar. The only grade a headline claim
    /// may carry. Derived from a `Gated` `LedgerRecord` with `passed == true`.
    Gated = 0,
    /// Computed from real, replayed or simulated input and recorded in the
    /// commit-keyed ledger. True by construction for any recorded result.
    Measured = 1,
    /// Asserted without a recorded measurement. Never emitted to Home Assistant.
    Claimed = 2,
    /// Blocked on missing input: no fixture, no labels, no reference recording,
    /// or insufficient paired samples.
    DataGated = 3,
    /// Blocked on missing hardware: a reference device, chest strap or belt,
    /// camera rig, or the reference-scale node deployment.
    HardwareGated = 4,
}
```

Only `Gated` and `Measured` may carry a value (`Witness::may_carry_value`);
`Claimed`, `DataGated` and `HardwareGated` are pending or unbacked claims.

**Derivation rule (defined once):** a runtime claim's `Witness` is `Gated` iff a
`LedgerRecord` with `kind == MetricKind::Gated` and `passed == true` backs it,
`Measured` iff any non-gated `LedgerRecord` backs it, and otherwise the claim is
unmeasured: `Claimed` for an asserted, unbacked claim, or the appropriate blocked
sub-grade — `DataGated` when the missing input is data, `HardwareGated` when the
missing input is hardware (`evaluation.md` §5.1–§5.2).

**Seam mapping** (contracts ↔ the six real seams): `CsiSource` yields
`CsiEnvelope`; `Clock` produces `ClockModel`; `PoseModel`/`VitalsModel` produce
`Pose`/`VitalsReading`; `GroundTruth` produces `GroundTruthRecord`; `Sink` consumes
`Detection`/`Event`; `MetricSink` consumes `LedgerRecord`.

---

## 2. `CsiFrame` — the raw sensing sample

One received 802.11 packet's raw channel state information from a single node:
the esp-csi **signed int8 I/Q** sample per subcarrier plus capture metadata. The
modem's native form, carried verbatim; the sample is interleaved
**imaginary-first** (`[Q0, I0, Q1, I1, …]`), as esp-csi emits it (ADR-0006).
Amplitude and phase are **derived** in the core, never stored (ADR-0005).

**Fields**

| Field | Type | Unit | Notes |
| --- | --- | --- | --- |
| `node_id` | `NodeId` | — | On-air identity; see §1 |
| `boot_epoch` | `u16` | — | A fresh value each boot from the node RNG; **not** persisted (`capture-and-firmware.md` §9.1.5). The core treats a change as a reboot |
| `sequence` | `u64` | count | Monotonic within `(node, boot_epoch, band, channel)` |
| `device_time_us` | `u64` | µs | Node-local monotonic; **not** synchronised |
| `band` | `Band` | — | 2.4 / 5 GHz / Unknown |
| `channel` | `u8` | channel | 1–14 (2.4 GHz); 32–177 (5 GHz) |
| `bandwidth_mhz` | `u8` | MHz | 20 or 40 |
| `subcarrier_offset` | `i16` | index | Index of the first interleaved sample below |
| `i_q` | `Vec<i8>` | raw counts | Interleaved `Q, I` per subcarrier (**imaginary first**), ascending index |
| `rssi_dbm` | `i8` | dBm | Whole-frame RSSI |
| `noise_floor_dbm` | `i8` | dBm | Whole-frame noise floor |
| `phy` | `PhyFlags` | bitset | See below |
| `first_word_invalid` | `bool` | — | ESP32 quirk: the leading complex sample (two bytes) is unreliable. Carried on the wire as header `flags` bit 0 (§9.1) |

`n_subcarriers` is **derived** (`i_q.len() / 2`) and is not a separate stored
field, so it cannot disagree with the sample. Amplitude (`√(I²+Q²)`) and phase
(`atan2(Q, I)`) are derived accessors on the frame, not stored fields; they read
the in-phase byte as the *second* byte of each pair and the quadrature byte as
the *first* (imaginary-first).

```rust
bitflags-like newtype (u16):
pub struct PhyFlags(pub u16);
// bit 0: HT (11n), bit 1: secondary present, bit 2: STBC, bit 3: LDPC,
// bit 4: short GI, bit 5: sounding/NDP, bit 6: IQ saturated (capture/simulator
// diagnostic), bit 7: VHT (11ac), bit 8: HE (11ax). Bits 9–15 reserved.
```

**Invariants (enforced by `aquila-sources` on ingest; violations are rejected and counted)**

1. `i_q.len()` is even and `>= 2` (`n_subcarriers >= 1`).
2. `i_q` is in **ascending subcarrier index** order, contiguous, starting at
   `subcarrier_offset`: `[Q0, I0, Q1, I1, …]` (imaginary first). Null/guard
   subcarriers may appear as the pair `(0, 0)`; downstream DSP decides whether to
   use them.
3. Every element is a signed int8, so its derived amplitude and phase are finite
   by construction; there is no range check to fail.
4. `device_time_us` is non-decreasing within a `boot_epoch`.
5. `sequence` strictly increases within a `(node, boot_epoch, band, channel)`
   stream modulo loss. Gaps quantify loss; duplicates (`sequence` seen twice) are
   dropped by the ingest deduplicator.
6. A reboot is detected when `boot_epoch` changes, or `device_time_us`/`sequence`
   goes backwards. On reboot the core opens a new epoch; it never splices across it.
7. Per-frame phase is **uncalibrated** (independent PLL/AGC phase per frame).
   Cross-frame phase comparison is out of scope for v1 and MUST NOT be inferred
   from the derived phase alone.

**Serialisation**

- **Wire (UDP):** fixed little-endian header, then body. See §9.
- **On disk:** inside a `.aqs` session archive, the **exact on-wire `AQCI`
  datagram bytes** plus a host-arrival stamp (§9.2, `capture-and-firmware.md`
  §8). `postcard` is **not** used for the frame stream.
- **JSON (fixtures/debug only):** `i8` samples as arrays of integers; never the
  production transport.

**Rust sketch**

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CsiFrame {
    pub node_id: NodeId,
    pub boot_epoch: u16,
    pub sequence: u64,
    pub device_time_us: u64,
    pub band: Band,
    pub channel: u8,
    pub bandwidth_mhz: u8,
    pub subcarrier_offset: i16,
    pub i_q: Vec<i8>, // interleaved Q, I (imaginary first) per subcarrier
    pub rssi_dbm: i8,
    pub noise_floor_dbm: i8,
    pub phy: PhyFlags,
    pub first_word_invalid: bool,
}
impl CsiFrame {
    pub fn n_subcarriers(&self) -> usize { self.i_q.len() / 2 }
    pub fn amplitude(&self) -> Vec<f32> { /* √(I²+Q²) */ }
    pub fn phase(&self) -> Vec<f32> { /* atan2(Q, I) */ }
}
```

**`CsiEnvelope`** — the core-side wrapper. The node frame stays pure; the core
adds arrival data using the `Clock` seam.

| Field | Type | Unit | Notes |
| --- | --- | --- | --- |
| `frame` | `CsiFrame` | — | Verbatim |
| `host_rx_mono_ns` | `u64` | ns | Arrival on the host monotonic clock |
| `host_rx_unix_ms` | `i64` | ms | Wall time at arrival (audit only) |
| `source_addr` | `Option<SocketAddr>` | — | For logging/diagnostics only |

### 2.1 `SensingFrame` — the core's atomic unit of analysis

The input to feature extraction and inference: the CSI frames of one node **or**
one node-pair, time-aligned onto a common clock grid over a fixed span. Not a new
kind of CSI — the same `CsiEnvelope`s, placed on a grid.

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensingFrame {
    pub scope: SensingScope,        // one node or one node-pair
    pub span: TimeSpan,
    pub grid: ClockGrid,            // the common clock grid
    pub frames: Vec<CsiEnvelope>,   // contributing CSI frames
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensingScope { Node(NodeId), Pair(PairId) }

/// Uniform sampling grid: slot k is at origin_mono_ns + k * interval_ns.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClockGrid { pub origin_mono_ns: u64, pub interval_ns: u64, pub slots: u32 }
```

**Invariants**: `frames` all belong to `scope`; each frame is assigned to the
grid slot whose span contains its synchronised arrival; `frames` are ordered by
`(host_rx_mono_ns, node_id, sequence)`. A `SensingFrame` never mixes nodes outside
a declared pair.

---

## 3. Home topology — `Home`, `Floor`, `Room`, `Zone`, `Node`, `NodePair`

The physical model, defined from day one: **Home → Floors → Rooms → Nodes →
NodePairs → Zones**. Stored as JSON in `config/topology.json` and loaded by the
`aquila-geometry` module. Rooms describe where nodes are physically installed;
zones are virtual detection regions — the unit of sensing output. Nodes, rooms
and zones are *configured*, not inferred. A zone may be coextensive with a
room, a sub-region of a room (for example "bed" inside the bedroom, or "shower"
versus "toilet" inside the bathroom), or span multiple rooms.

### 3.1 Coordinate model (one frame, no ambiguity)

- **Home frame**: Cartesian, metres, right-handed, `x` east, `y` north, `z` up.
  Origin at the home's declared reference point.
- **Floor**: carries `elevation_m` — its plane's height in the home frame.
- **Room**: carries `origin_home_m` and `yaw_deg` mapping room-local → home frame.
- **Node**: carries `pos_room_m`, room-local metres.
- **Pose** (§4) is expressed in **room-local metres**.
- Positions are `Option`. `None` means "not yet surveyed"; pose and fusion must
  degrade honestly rather than invent geometry. Global calibration (§10) fills
  and refines these.

### 3.2 Types

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Home {
    pub id: HomeId,
    pub name: String,
    pub time_zone: String,             // IANA, e.g. "Australia/Sydney"
    pub contract: ContractVersion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Floor {
    pub id: FloorId,
    pub home: HomeId,
    pub name: String,
    pub level: i8,                     // -1 basement, 0 ground, 1, 2 ...
    pub elevation_m: f32,              // height of this floor plane in home frame
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Room {
    pub id: RoomId,
    pub floor: FloorId,
    pub name: String,
    pub kind: RoomKind,
    pub origin_home_m: [f32; 3],
    pub yaw_deg: f32,
    pub area_m2: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomKind {
    Living, Bedroom, Kitchen, Dining, Bathroom, Hall, Stairs, Office, Utility,
    #[serde(other)] Unknown = 255,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub room: RoomId,
    pub name: String,
    pub mac: MacAddr,
    pub device: DeviceType,
    pub firmware: FirmwareRef,
    pub pos_room_m: Option<[f32; 3]>,  // room-local metres; None until surveyed
    pub installed_unix_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType { Esp32S3, Esp32C6, #[serde(other)] Unknown = 255 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FirmwareRef { pub version: String, pub sha256: [u8; 32] }

/// An **ordered** node-pair: the unit of spatial sensing and the edge of the
/// sensing graph. `tx` transmits, `rx` measures the channel from it. Direction
/// is significant (CSI is directional). v1 stages pairs: within-room first,
/// then within-floor, then cross-floor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodePair {
    pub id: PairId,
    pub home: HomeId,
    pub tx: NodeId,
    pub rx: NodeId,
    pub scope: PairScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairScope { Room, SameFloor, CrossFloor }

/// A virtual detection region. A zone may be coextensive with a room (zone
/// `lounge` inside room `lounge`), a sub-region of a room (zone `bed` inside
/// room `bedroom`, or zone `shower` vs zone `toilet` inside room `bathroom`),
/// or span multiple rooms (`room` is `None`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub id: ZoneId,
    /// The room this zone sits within, when it is a sub-region of one room.
    /// `None` when the zone spans rooms or covers the whole home.
    pub room: Option<RoomId>,
    pub name: String,
}

/// The whole configured model. Referential integrity is an invariant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Topology {
    pub home: Home,
    pub floors: Vec<Floor>,
    pub rooms: Vec<Room>,
    pub zones: Vec<Zone>,
    pub nodes: Vec<Node>,
    pub pairs: Vec<NodePair>,
}
```

**Invariants**

1. All `*Id` values are unique within their collection and non-empty.
2. Every `Floor.home` matches `Home.id`; every `Room.floor` exists; every
   `Zone.room`, when `Some`, exists; every `Node.room` exists; every
   `NodePair.tx`/`.rx` exist.
3. `NodePair.tx != NodePair.rx`; the **ordered** pair `(tx, rx)` is unique (one
   directed sensing link per ordered pair per Home in v1).
4. `PairScope` is **derived**, not free input: `Room` iff `tx` and `rx` share a
   room, `SameFloor` iff they share a floor but not a room, else `CrossFloor`. A
   stored `scope` that disagrees with the topology is a load error.
5. `level` values within a Home are unique and ordered ascending.
6. `NodeId::UNPROVISIONED` never appears in a committed topology.

**Serialisation** — JSON (serde), pretty-printed, keys in declaration order, at
`config/topology.json`. Human-editable, diffable, version-pinned by `Home.contract`.

---

## 4. `Pose` — 17 keypoints and confidence

A coarse, zone-scoped configuration of body keypoints for one subject at one
instant. Low-fidelity by design; v1 gated metric: in-domain torso-PCK@20 ≥ 65 %.
The keypoint schema is **COCO-17**, fixed, and **never redefined** (§8).

```rust
pub const KEYPOINT_COUNT: usize = 17;

/// Fixed order. Discriminants are the wire order; do not renumber.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeypointId {
    Nose = 0, LeftEye, RightEye, LeftEar, RightEar,
    LeftShoulder, RightShoulder, LeftElbow, RightElbow,
    LeftWrist, RightWrist, LeftHip, RightHip,
    LeftKnee, RightKnee, LeftAnkle, RightAnkle,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Keypoint {
    pub id: KeypointId,
    pub x_m: f32,          // room-local metres
    pub y_m: f32,          // room-local metres
    pub z_m: Option<f32>,  // room-local metres; None if the model is planar
    pub confidence: f32,   // [0, 1]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    pub subject: SubjectId,
    pub room: RoomId,                        // the zone in v1
    pub t_unix_ms: i64,
    pub keys: [Keypoint; KEYPOINT_COUNT],    // exactly 17, ascending KeypointId
    pub score: f32,                          // whole-subject confidence [0, 1]
    pub witness: Witness,
}
```

**Invariants**

1. `keys.len() == 17` and `keys[i].id as u8 == i` — the array *is* the schema.
2. `confidence ∈ [0, 1]` and `score ∈ [0, 1]`.
3. A missing keypoint is expressed as `confidence == 0.0` with `x_m`/`y_m` **not**
   relied upon (they may be the model's best guess; consumers must gate on
   `confidence`). `x_m`/`y_m` may be `NaN` to mean "no estimate".
4. Coordinates are room-local metres per §3.1. `z_m` is `None` for planar v1 poses.
5. A pose whose `score` is below the configured quality gate is withheld from the
   `Sink` but is still recorded for scoring. Withholding is a gate, not a
   `Witness` value.

**Serialisation** — serde; a typed `0x02` ground-truth record inside the `.aqs`
session archive (exact wire bytes only for CSI frames); JSON for fixtures.
Keypoint order is authoritative; never emit a map keyed by name.

---

## 5. `VitalsReading` — breathing rate, heart rate, status, witness

A windowed physiological estimate for one subject (or one zone when no subject is
available). Breathing rate and heart rate are **separate** estimates with their
own confidence; either may be absent. No HRV, SpO2 or rhythm analysis.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Estimate {
    pub value: f32,        // units per the field that carries it
    pub confidence: f32,   // [0, 1]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VitalsStatus {
    Ok,
    InsufficientSignal,    // SNR below the estimability floor
    MotionArtifact,        // movement corrupts the phase history
    NoSubject,             // nobody present in scope
    CalibrationStale,      // calibration older than the model permits
    #[serde(other)] Unknown = 255,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VitalsReading {
    pub subject: Option<SubjectId>,   // None = zone-scoped reading
    pub room: RoomId,                 // the zone in v1
    pub span: TimeSpan,
    pub breathing: Option<Estimate>,  // value unit: breaths per minute (brpm)
    pub heart_rate: Option<Estimate>, // value unit: beats per minute (bpm)
    pub status: VitalsStatus,
    pub witness: Witness,
}
```

**Invariants**

1. `breathing.value` is brpm, plausible range `[4, 60]`; `heart_rate.value` is
   bpm, plausible range `[30, 220]`. Values outside range are dropped (estimate
   set to `None`) with a `MotionArtifact`/`Unknown` status and a ledger note —
   never emitted.
2. `status == Ok` requires at least one of `breathing`/`heart_rate` to be `Some`.
3. `status != Ok` requires the affected estimate(s) to be `None`. A reading never
   simultaneously claims "no signal" and reports a value.
4. Readings with `status != Ok`, or `witness == Claimed`, are recorded but not
   surfaced to Home Assistant.
5. `span.end_mono_ns > span.start_mono_ns`.
6. `confidence` is the model's own certainty; `witness` is evidence strength.
   They are independent and both are carried.

**Serialisation** — serde; a typed `0x02` record inside the `.aqs` session
archive; JSON for fixtures/ingress.

---

## 6. `Detection` and `Event`

Two distinct concepts, deliberately not merged:

- **`Detection`** — a *stateless* inference for a zone (or the home) over a span.
  Emitted every span; may say "no change".
- **`Event`** — an *edge-triggered, deduplicated* state change destined for the
  `Sink` (Home Assistant). Emitted only when state actually changes, after
  hysteresis.

A `Detection` is the evidence; an `Event` is the notification. One `Event`
references the `Detection`s that justified it.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionKind {
    Presence,   // per-zone binary claim that at least one person is present
    Count,      // people attributed to a zone or the home
    Occupancy,  // whole-home fused state of which zones are occupied and by how many
    #[serde(other)] Unknown = 255,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Detection {
    pub scope: DetectionScope,        // Room (zone) or Home
    pub span: TimeSpan,
    pub kind: DetectionKind,
    pub confidence: f32,              // [0, 1]
    pub value: DetectionValue,
    pub contributors: Vec<NodeId>,    // nodes whose CSI fed this detection, non-empty
    pub witness: Witness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionScope { Home(HomeId), Room(RoomId) }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionValue {
    Probability(f32),                 // Presence
    Count(u8),                        // Count
    Occupancy(Vec<(RoomId, u8)>),     // Occupancy: per-zone people counts
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    PresenceChanged,
    CountChanged,
    OccupancyChanged,
    PersonEntered,
    PersonLeft,
    VitalAlert,
    CalibrationChanged,
    NodeOnline,
    NodeOffline,
    ModelChanged,
    #[serde(other)] Unknown = 255,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub id: String,                   // deterministic: hash(home, kind, scope, t_slot)
    pub home: HomeId,
    pub t_unix_ms: i64,
    pub dedup_key: String,            // f"{home}:{kind}:{scope}"
    pub scope: EventScope,
    pub kind: EventKind,
    pub payload: EventPayload,        // typed per kind; see below
    pub witness: Witness,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventScope { Home, Floor(FloorId), Room(RoomId) }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventPayload {
    Presence { occupied: bool, probability: f32 },
    Count { people: u8 },
    Occupancy { by_zone: Vec<(RoomId, u8)> },
    Vitals { metric: String, value: f32, unit: String },
    Node { node: NodeId, reason: String },
    Change { summary: String },
    None,
}
```

**Detection invariants**

1. `contributors` is non-empty; every element exists in the topology.
2. `DetectionValue::Probability(p)` → `p ∈ [0, 1]`.
3. `DetectionValue::Count(n)` → `n` is a small integer (v1 caps at 8).
4. `value` variant must match `kind` and `scope`: `Presence` → `Probability` and a
   `Room` scope; `Count` → `Count`; `Occupancy` → `Occupancy` and a `Home` scope.
5. A detection whose `confidence` is below the gate is withheld from the Event
   pipeline but still recorded.

**Event invariants**

1. `id` and `dedup_key` are deterministic functions of their inputs (no clock, no
   RNG) so replays are byte-identical.
2. **Hysteresis**: an `Event` is emitted only after the underlying detection has
   held for the configured dwell time (default 2 s) and crossed the configured
   thresholds. Thresholds live in config, and the values used are pinned in the
   `LedgerRecord` (§7).
3. `witness == Claimed` never yields an `Event`; it is filtered at the `Sink`.
4. `dedup_key` collisions are resolved "first wins"; a repeated key within the
   dwell window is dropped, not emitted twice.

**Serialisation** — `Detection` and `Event` use serde; an `Event` is delivered to
the `Sink` as JSON over the add-on's private channel, and recorded in the
session's event stream as JSONL.

---

## 7. `Session` and the measurement ledger

### 7.1 `Session`

A `Session` is a recorded, replayable, time-bounded stream of CSI frames from one
or more nodes, with metadata and provenance. Sessions drive development and
scoring ahead of live deployment. It is *not* the add-on's uptime; live operation
is a sequence of sessions. Same inputs, same seed, same session ⇒ same metrics.

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub name: String,
    pub home: HomeId,
    pub nodes: Vec<NodeId>,
    pub started_unix_ms: i64,
    pub ended_unix_ms: Option<i64>,
    pub contract: ContractVersion,
    pub build: BuildId,
    pub config_hash: [u8; 32],        // sha256 of canonical config
    pub seed: u64,                    // RNG seed for any stochastic step
    pub artifacts: Vec<SessionArtifact>,
    pub ground_truth: Option<GroundTruthRef>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildId { pub git_sha: String, pub dirty: bool }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionArtifact { pub path: String, pub sha256: [u8; 32], pub frames: u64 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundTruthRef { pub path: String, pub sha256: [u8; 32], pub kind: GtKind }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GtKind { CameraResearch, ReferenceSensor, Manual, Synthetic }
```

**Invariants**

1. `ended_unix_ms >= started_unix_ms` when set.
2. `nodes` is non-empty and every node exists in the topology of `home`.
3. `contract` is pinned; a session recorded under an older contract is readable
   only through an explicit migration, never silently reinterpreted.
4. `config_hash` and `seed` fully determine any stochastic output.

**`GroundTruthRecord`** (research-only seam; never a runtime input):

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundTruthRecord {
    pub session: SessionId,
    pub t_unix_ms: i64,
    pub kind: GtKind,
    pub scope: EventScope,
    pub payload: EventPayload,   // reuse; e.g. Presence/Count
}
```

### 7.2 `LedgerRecord` — commit-keyed measurement

Measurement is first-class: **record and diff, no CI gating yet.** Every metric
result is appended to `.aquila/metrics/ledger.jsonl` as one `LedgerRecord` per
line.

`LedgerRecord` is the **frozen, on-disk ledger row** and the only canonical one
(§8). `aquila-metrics` also carries an in-memory `Measurement` record
(`measurement.md` §2) that serialises into a `LedgerRecord`; where this document
and `measurement.md` describe the same field the frozen name here wins.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    Deterministic,  // must reproduce bit-identically for identical inputs+env
    Statistical,    // characterised by a distribution; carries n and a CI
    Gated,          // compared against a v1 bar; carries pass/fail
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Interval { pub low: f64, pub high: f64 }   // invariant: low <= high

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetRef { pub name: String, pub split: Split }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Split { Train, Validation, Test, InDomain, OutOfDomain }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRef { pub id: String, pub sha256: [u8; 32], pub onnx_opset: u32 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvFingerprint {
    pub host: String,
    pub rustc: String,
    pub os: String,
    pub thread_count: u32,
    pub rng: String,                 // e.g. "splitmix64:seed"
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerRecord {
    pub id: String,                  // deterministic: hash(commit, session, metric, config, dataset)
    pub commit: BuildId,
    pub contract: ContractVersion,
    pub session: SessionId,
    pub metric: MetricId,
    pub kind: MetricKind,
    pub value: f64,
    pub unit: String,                // e.g. "PCK@20", "F1", "brpm"
    pub samples: u64,
    pub ci: Option<Interval>,        // required for Statistical
    pub dataset: DatasetRef,
    pub model: Option<ModelRef>,
    pub config_hash: [u8; 32],
    pub threshold: Option<f64>,      // required for Gated
    pub passed: Option<bool>,        // required for Gated
    pub env: EnvFingerprint,
    pub created_unix_ms: i64,
    pub supersedes: Option<String>,  // prior record id, for diffing history
}
```

**Invariants**

1. Append-only. Records are never edited or deleted; a re-measurement supersedes
   via `supersedes`.
2. `id` is a pure hash of its inputs (no clock, no RNG); re-running identical
   inputs yields an identical `id` and value.
3. `MetricKind::Deterministic` records must reproduce bit-identically for the same
   `(commit, config_hash, session, env)`. A mismatch is a bug, recorded as a
   contradicting record, not overwritten.
4. `MetricKind::Statistical` requires `ci` and `samples > 0`.
5. `MetricKind::Gated` requires `threshold` and `passed`.
6. `unit` is a stable label; the v1 bar labels are `PCK@20`, `F1`, `brpm`.
7. `commit.dirty == true` records are admissible but flagged; they are not
   comparable across machines without the same working tree.
8. A claim's `Witness` (§1) is derived from these records; the ledger is the only
   source of `Gated`/`Measured`.

**Serialisation** — JSON Lines, one record per line, keys in declaration order,
UTF-8, LF endings. Diffable with `git diff --word-diff`. No pretty-printing.

### 7.3 The v1 bars (context)

Recorded as `MetricKind::Gated` records, not CI gates. The **canonical metric-id
vocabulary** for the three v1 gated bars — the single set every spec and the
ledger cite — is:

| Bar | Canonical metric id | Unit | Direction |
| --- | --- | --- | --- |
| In-domain torso PCK@20 (≥ 65 %) | `accuracy.torso_pck_at_20` | % | higher is better |
| Presence F1 (≥ 0.9) | `accuracy.presence_f1` | ratio | higher is better |
| Breathing rate MAE (≤ 2 brpm) | `accuracy.breathing.mae_brpm` | brpm | lower is better |

Heart rate (`accuracy.heart_rate.mae_bpm`) is reported but ungated in v1. These
are the ids `measurement.md` §4.6 owns and `evaluation.md` §6 and `core-api.md`
§13 render; a differently spelled id is a defect. A claim without a
`LedgerRecord` is `Claimed`, and no headline claim may carry that grade.

---

## 8. Types that must NEVER be redefined

These are frozen names owned by `aquila-types`. A second declaration anywhere
(in the firmware, a Python model I/O class, an adapter, a test fixture type) is a
defect:

1. **`CsiFrame`** — and its subcarrier representation (raw signed int8 `i_q`,
   interleaved `Q, I` — **imaginary first** — ascending index), plus the
   `first_word_invalid` impairment flag. Amplitude and phase are derived, never
   stored. The firmware emits the wire form; it does not own a competing struct.
2. **`CsiEnvelope`**, **`SensingFrame`**, `SensingScope`, `ClockGrid` — the only
   core-side wrappers; do not add second versions.
3. **`NodeId`**, `HomeId`/`FloorId`/`RoomId`/`ZoneId`/`PairId`, `MacAddr`,
   `SubjectId`.
4. **`Band`**, **`SecondaryChannel`**, **`PhyFlags`**.
5. **`KeypointId`** and the **COCO-17 `Keypoint`/`Pose` schema** — order and count
   are frozen; the keypoint vocabulary is COCO-17 and nothing else.
6. **`VitalsReading`**, `Estimate`, `VitalsStatus`.
7. **`Detection`**, **`Event`**, `DetectionKind`, `EventKind`, `EventPayload`.
8. **`Session`**, `BuildId`, `SessionArtifact`, `GroundTruthRef`.
9. **`LedgerRecord`**, `MetricKind`, **`Witness`**, `ContractVersion`.
10. **`Topology`** and its entities `Home`/`Floor`/`Room`/`Node`/`NodePair`.
11. **`Calibration`**, **`Baseline`**, `ClockModel`.

Rule of thumb: if RuView needed to define it twice, it is on this list. Add fields
(via §0.1 minor bumps); never fork.

---

## 9. Serialisation in full

### 9.1 Wire format over UDP (canonical on-air)

Little-endian, packed, no trailing padding. Max datagram 1472 bytes (stay under a
1500-byte MTU); oversized frames are dropped and counted, never fragmented at the
application layer.

**Fixed header — 16 bytes**

| Offset | Size | Field | Encoding |
| --- | --- | --- | --- |
| 0 | 4 | magic | ASCII `AQCI` |
| 4 | 1 | `wire_major` | `u8` |
| 5 | 1 | `wire_minor` | `u8` |
| 6 | 1 | `msg_type` | `u8` (1 = `CsiFrame`) |
| 7 | 1 | `sc_format` | `u8` (1 = raw signed int8 I/Q, imaginary first) |
| 8 | 2 | `node_id` | `u16` |
| 10 | 2 | `payload_len` | `u16` (bytes after the header) |
| 12 | 1 | `band` | `u8` (0 = 2.4 GHz, 1 = 5 GHz, 255 = unknown) |
| 13 | 1 | `channel` | `u8` |
| 14 | 1 | `bandwidth_mhz` | `u8` |
| 15 | 1 | `flags` | `u8`; bit 0 = `first_word_invalid` (leading complex sample unreliable); other bits reserved = 0 |

**Body (msg_type 1) — 34-byte fixed part (wire minor 1), then `2 × n` bytes**

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | `sequence` |
| 8 | 8 | `device_time_us` |
| 16 | 2 | `boot_epoch` |
| 18 | 2 | `subcarrier_offset` (`i16`) |
| 20 | 2 | `n_subcarriers` (`u16`) |
| 22 | 1 | `rssi_dbm` (`i8`) |
| 23 | 1 | `noise_floor_dbm` (`i8`) |
| 24 | 2 | `phy_flags` (`u16`) |
| 26 | 2 | `rx_seq` — the 802.11 sequence number of the received frame (`u16`); `0xFFFF` means "not reported" |
| 28 | 6 | `source_mac` — the transmitting station's MAC address (wire minor 1 and later; all zeroes means "transmitter unknown") |
| 34 | 2n | `i_q[0..2n]` (interleaved signed int8 `Q, I` per subcarrier, imaginary first) |

Rules: a receiver validates `magic`, `wire_major`, `payload_len` and
`payload_len == 34 + 2 * n_subcarriers` before trusting any field. `wire_minor`
mismatches are accepted (additive); `wire_major` mismatches are rejected and
counted once per `(node_id, boot_epoch)`. The codec knows each minor's body
layout: wire minor 0 carries no `source_mac` and its fixed body is 28 bytes, so
`payload_len == 28 + 2 * n_subcarriers`; wire minor 1 and later carry the
six-byte `source_mac`. The `magic` is `AQCI` and the payload
is the raw signed int8 I/Q sample, **imaginary byte first** (ADR-0005,
ADR-0006). Header `flags` bit 0 (`first_word_invalid`) marks a frame whose
leading complex sample (two bytes) the driver reported unreliable. The
`aquila-wire` crate provides `encode_datagram`/`decode_datagram`; `postcard` is
**not** used on the wire.

### 9.2 On disk

| Artefact | Format | Location |
| --- | --- | --- |
| Topology | JSON, pretty, declaration-order keys | `config/topology.json` |
| Session archive | `.aqs` single-file container (AQS/AQE; below) | `sessions/<session-id>.aqs` |
| Ledger | JSONL, canonical key order | `.aquila/metrics/ledger.jsonl` |
| Ground truth | typed `0x02` record inside the `.aqs` container | `sessions/<session-id>.aqs` |
| Event stream | typed `0x03` record inside the `.aqs` container | `sessions/<session-id>.aqs` |

**`.aqs` session container.** The durable session representation is the
`aquila-session` container (`capture-and-firmware.md` §8 is authoritative for the
byte layout): a fixed AQS header, a canonical-JSON typed manifest, a stream of
typed, length-prefixed records, and an AQE digest trailer.

```
"AQS" magic | u8 version | u32 metadata_len (LE) | metadata (canonical JSON)
record*    (u8 type | u32 body_len LE | body)
"AQE"       | u64 record_count (LE) | sha256(record stream)
```

A CSI-frame record (`0x01`) stores a `u64 host_arrival_us` followed by the
**exact on-wire `AQCI` datagram bytes**, so replay runs the real `aquila-wire`
decoder and the codec cannot drift from live. Records `0x02` (ground truth),
`0x03` (event) and `0x04` (annotation) carry JSON bodies. The container-level
manifest (`aquila-session/1`) wraps the frozen `Session` contract (§7.1) with the
capture-tool, core-version, elicitation and licence facts; it is **not** a
second contract type. `postcard` is not used anywhere in the session container.
JSON is used wherever humans diff, edit or debug.

---

## 10. Time, calibration, baseline and their contracts

**Units bridge (µs ↔ ns).** The wire and contract carry `device_time_us` in
**microseconds** (§2); the core's `CoreTime`/`DeviceTime` and the time-sync maths
(`fusion-sync.md` §1) are in **nanoseconds**. The one conversion is
`ns = us × 1000` at the ingest seam; it is stated here once so offset and drift
maths is never ambiguous.

**`Calibration` (canonical).** `aquila_types::Calibration` below is the **one**
runtime contract named `Calibration`; `data-contracts.md` §8 freezes the name.
`aquila-signal`'s per-node multiplier is an internal type (renamed
`NodeCorrection` in `signal.md` §2, pending the same rename in the crate), and the
on-disk calibration artefact in `geometry-calibration.md` §13 is a distinct
serialisation (`CalibrationArtefact`) that cites this contract. Neither is a second
`Calibration`.

```rust
/// Per-node software time-sync estimate produced by the Clock seam.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClockModel {
    pub node: NodeId,
    pub boot_epoch: u16,
    /// host_mono_ns = device_time_us*1000 + offset_ns + drift_ppm*(elapsed)
    pub offset_ns: i64,
    pub drift_ppm: f64,
    pub uncertainty_ns: u64,
    pub estimated_mono_ns: u64,
    pub samples: u64,
    pub witness: Witness,
}

/// Global, whole-home calibration: the per-home process that corrects node and
/// node-pair CSI to a common reference, and the stored corrections it produces.
/// v1 is global — one calibration for the whole home — not per-room.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub home: HomeId,
    pub contract: ContractVersion,
    pub created_unix_ms: i64,
    pub session: SessionId,
    pub topology_hash: [u8; 32],                    // bound to the topology it corrects
    pub node_positions_m: Vec<(NodeId, [f32; 3])>,  // home-frame metres
    pub node_offsets_db: Vec<(NodeId, f32)>,        // per-node gain correction
    pub pair_offsets: Vec<(PairId, PairCorrection)>,
    pub validity: TimeSpan,
    pub witness: Witness,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PairCorrection { pub amplitude_db: f32, pub phase_rad: f32 }

/// A reference state recorded with no person present, against which deviations
/// (presence, vitals) are measured. One per scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    pub home: HomeId,
    pub session: SessionId,
    pub scope: BaselineScope,
    pub span: TimeSpan,
    pub amplitude_ref_db: Vec<f32>,   // per subcarrier
    pub phase_ref_rad: Vec<f32>,      // per subcarrier (uncalibrated, same node)
    pub created_unix_ms: i64,
    pub witness: Witness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineScope { Zone(RoomId), Pair(PairId) }
```

Invariants: `ClockModel.uncertainty_ns` is a bound the core publishes and honours.
A calibration is bound to `topology_hash`; a topology change invalidates it and
emits `CalibrationChanged`. `VitalsReading.status == CalibrationStale` follows
when no `Calibration` of adequate `Witness` covers the reading's span. A
`Baseline` is invalidated by a topology or calibration change that alters its
scope. Per-room and per-floor calibration are staged later; they extend this
contract additively, they do not replace it.

---

## 11. Change checklist (for any contract edit)

1. Decide MINOR or MAJOR with §0.1.
2. Update this document **first** (fields, units, invariants, serialisation).
3. Check the change against `CONTEXT.md` vocabulary; use its terms and no others.
4. Update `aquila-types`; keep derives and defaults consistent with §0.3.
5. If the wire changed, bump `wire_minor`/`wire_major` and update §9.1.
6. Add a fixture in `aquila-types` that round-trips the new shape and pins it.
7. If MAJOR: add a migration note and update `CONTRACT_VERSION`.
8. Never touch a name on the §8 frozen list except by a MAJOR bump.

_This document is canonical. Where it and any code disagree, this document and the
`aquila-types` crate are the tie-breakers — and if they disagree with each
other, that is a bug._
