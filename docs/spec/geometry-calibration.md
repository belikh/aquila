# Geometry & global calibration — specification

Status: draft for review. Scope: v1 (sensing core + global whole-home
calibration). Owner role: geometry & calibration author.

This spec defines two things and their seam:

1. **Geometry** — how Aquila models *where things are*: the home topology, the
   coordinate frames, how nodes are placed, how observations are attributed to
   a floor → room → zone, and what position can honestly be inferred from
   incoherent nodes.
2. **Global whole-home calibration** — what is learned, how the dashboard
   wizard triggers it, the artefact it produces, where that artefact lives, how
   it is versioned and invalidated, and how its quality is measured.

It is written against the settled brief (Aquila, `aquila-<descriptor>` crates,
Home Assistant host, Linux-only, ESP32-S3 reference, ~9+ nodes over 3 floors,
raw CSI over WiFi UDP, no ESP-NOW, no MQTT, in-tree only). It does not reopen
those decisions.

---

## 0. The hard limit, stated first

Each ESP32 has its **own local oscillator and sampling clock**. There is no
shared reference, no hardware timestamp exchange, and no phase-locked carrier.
The control plane is a *software* time sync (clock-offset + skew estimation)
over UDP. Consequences that bind the whole design:

- CSI **phase is usable only within one node's one measurement**, across that
  measurement's own subcarriers (relative delay/spread). It is **not coherent
  across packets on one node**, and **never coherent across nodes**.
- Therefore there is **no cross-node array gain, no cross-node angle-of-arrival,
  and no ambiguity-free time-of-flight**. A phase-coherent RF reference is
  future-only and explicitly out of scope here.
- Indoor RSSI is multipath- and obstruction-biased; a path-loss "range" is
  accurate to a factor of roughly two, at best. Trilateration indoors with so
  few, incoherent anchors produces error ellipses often larger than the room.

So geometry in v1 is **approximate and confidence-scored**. The guaranteed
resolution is **room-level**, with **best-effort zone-level** and **advisory-only
sub-room position**. Every geometric output carries a confidence and a stated
basis; nothing downstream may treat an advisory position as a hard input.

---

## 1. Home topology (the domain model)

Owned by the `aquila-geometry` crate as its public data model. Present from day
one, in this hierarchy:

```
Home
└── Floor        (F0, F1, F2 …)          index + elevation
    └── Room     (living, kitchen, bed)  floor-plan polygon in floor frame
        └── Zone (sofa, bed, doorway)    named sub-volume in room frame
            └── Node (n00 … nNN)         one ESP32-S3 mount
                └── NodePair (link)      unordered node pair = one CSI link
```

- A **Node** belongs to exactly one Room (its mount), optionally to a Zone.
  A node with no measured position is first-class: room known, position
  `Unknown`.
- A **NodePair** (link) is the unit of channel observation: the unordered pair
  `{a, b}` with a canonical id (`a-b`, lexicographic). Its **scope** is derived,
  never stored as truth: `intra_room` (same room), `inter_room` (same floor),
  `inter_floor` (different floors).
- A **Zone** does not own nodes; it owns *contributing links* and thresholds.
  This keeps presence attribution a fusion problem, not a routing problem.

Topology is authored by the user in the wizard and is the spine the calibration
artefact hangs off. Topology changes are the primary calibration invalidation
(§10).

---

## 2. Coordinate frames

A `Frame` is a named, right-handed, metric coordinate system. Transforms between
frames are SE(2) (planar, per floor) or SE(3) for the world link, each carrying
a `Confidence`.

| Frame | Axes / origin | Units | Confidence source |
| --- | --- | --- | --- |
| `World` | ENU-ish: x east, y north, z up. Origin = survey reference (default: floor-0 room-0 corner, or the first mounted node). | metre | fixed (definition) |
| `Floor(F)` | planar x/y in the world floor plan; z fixed at the floor's elevation | metre | floor index (certain) |
| `Room(R)` | 2D local; origin at a chosen room corner, x along a chosen wall | metre | user-drawn polygon |
| `Node(N)` | mount frame; yaw optional; antenna orientation hint | metre / rad | placement, often `Unknown` |
| `CsiBand` | *not physical*: subcarrier index space for one node's snapshot | index | fixed |

Rules:

- **`Confidence` algebra** is explicit and tested: chain composition multiplies
  confidences (a transform through an unsure placement is itself unsure);
  conjunction takes the minimum; independent evidence combines by a noisy-OR.
  Monotonicity (more evidence never lowers confidence) is a property test.
- `Unknown` placement yields the identity transform with confidence `0.0`, not a
  guessed one. Geometry code must branch on `Unknown`, not substitute a default.
- Round-trip (`A→B→A`) and associativity are property-tested within tolerance.

The frames are a deep module: callers ask "give me this room's point in floor
coordinates, with a confidence", never juggling raw matrices.

---

## 3. Node placement model

```rust
struct NodePlacement {
    node_id: NodeId,
    floor_id: FloorId,           // required
    room_id: Option<RoomId>,     // None => floor-only, "unassigned"
    zone_id: Option<ZoneId>,
    position: Option<RoomPoint>, // None => Unknown
    yaw: Option<YawRad>,         // None => Unknown
    mounting: Mounting,          // Wall | Ceiling | Desk | Shelf | Unknown
    source: PlacementSource,     // User | Surveyed | Inferred
    confidence: Confidence,
}
```

The wizard collects `room_id` (required) and, optionally, a rough position the
user taps on a floor plan. Most users will supply room membership and skip
metric placement — the model must make that the happy path, with geometry
degrading gracefully to room-level.

---

## 4. Zone / room / floor mapping (attribution)

Given a set of link observations from a window, geometry produces a
**confidence-scored attribution**:

- `room_of(obs) -> Scored<RoomId>` — the room the perturbation most likely came
  from, by aggregating per-link zone signature likelihoods (§8, L3).
- `zone_of(obs) -> Scored<Option<ZoneId>>` — best-effort; `None` when no zone
  separates cleanly.
- `floor_of(obs) -> Scored<FloorId>` — via topology scope and a learned slab
  attenuation factor; lowest confidence, staged last.

Aggregation is feature-level fusion: each contributing link votes with a weight
learned at calibration; votes are combined as log-likelihoods and normalised to
a confidence. If the top two rooms are within a margin, attribution returns an
**ambiguous** result rather than a false confident one.

### Staging

| Stage | Resolution delivered | Prerequisite |
| --- | --- | --- |
| 1 | intra-room: room + zone | per-room baseline (≥1 usable link) |
| 2 | intra-floor: room→room separation, floor attribution | per-floor baselines + adjacent-room signatures |
| 3 | cross-floor: slab attenuation, inter-floor links | inter-floor baselines, staged after 1–2 |

Each stage is independently shippable and independently measured.

---

## 5. Proximity and trilateration bounds

Two estimators, in priority order.

**Primary — proximity fingerprinting (robust).** For each link, the
baseline-subtracted per-subcarrier amplitude vector over a window is a feature.
During the wizard walk-through the user labels zones; geometry learns a per-zone
signature (mean + covariance) per link, then classifies. This yields a discrete
proximity relation per link — `same_zone_candidate | adjacent | far` — and the
room/zone attribution above. It makes **no metric distance claim**, so multipath
cannot turn it into a wrong number, only a lower confidence.

**Secondary — RSSI trilateration (bounded, advisory).** With ≥3 placed anchors
and a fitted path-loss exponent `n` (default 2.0–3.5, refined only from links
with known placements), solve a weighted least-squares position and report its
**95% error ellipse** from the linearised Jacobian and per-link variances. This
is emitted as `PositionEstimate`, always accompanied by:

- `range_sigma_m` per link (honest per-link uncertainty), and
- `error_ellipse_95_m: [major, minor, orientation]`.

If the 95% ellipse exceeds the room polygon, the estimator **demotes itself to
room-level** and the continuous coordinate is marked `advisory`. It is never a
hard input to presence, pose, or vitals.

**Stated bounds table (contract, asserted in tests):**

| Claim | v1 support |
| --- | --- |
| Room membership | guaranteed given a usable baseline, else `Unknown` |
| Zone membership | best-effort; may return ambiguous/`None` |
| Continuous (x, y) in room | advisory only; 95% ellipse often ≥ room size |
| Cross-floor discrimination | staged; lowest confidence |
| Cross-node AoA / ToF / coherent gain | **not supported** (no phase coherence) |
| Relative phase within one node snapshot | supported (per-node delay/spread only) |

---

## 6. Geometry module, seams, and dependency DAG

### Crates

- **`aquila-geometry`** — topology model, frames/transforms, placement,
  zone/room/floor mapping, proximity fingerprint, bounded trilateration,
  `Confidence`. Pure computation, no I/O, no async. Depends only on a tiny
  `aquila-units` (metre, radian, dB) if needed.
- **`aquila-calibration`** — the whole-home calibration process: capture
  planning, baseline/threshold/drift/signature learning, quality gates, the
  artefact, versioning and invalidation policy. Depends on `aquila-geometry`,
  the DSP feature layer, and `aquila-metrics`.

### Seams (reused; no new ports invented)

Calibration and geometry use exactly the brief's seams:

| Seam | Role here |
| --- | --- |
| `CsiSource` | baseline capture stream; in tests, a `FakeCsiSource` replaying recorded fixtures |
| `Clock` | software time sync: offsets, skew, drift residual feeding the drift model |
| `GroundTruth` | wizard prompts ("stand in the kitchen") and QA labels |
| `Sink` | atomic write of the calibration artefact and the ledger entry |
| `MetricSink` | quality metrics into the commit-keyed ledger |

Reading an artefact back is a **pure function over bytes** (`parse_artefact`,
`validate_artefact`), so replay tests load recorded fixtures with no new seam.
Persistence *semantics* (content addressing, the `current` pointer) are owned by
`aquila-calibration`; the byte write goes through `Sink`.

### Enforced DAG

```
units ← geometry ← calibration ← { dsp, metrics }
```

No upward edges. Geometry never depends on the model crates. Enforced by the
repo's dependency-check tooling (per the brief's dependency-DAG rule).

### Public surface (sketch)

```rust
// aquila-geometry
pub struct Home { /* floors, rooms, zones, nodes, links */ }
pub enum Frame { World, Floor(FloorId), Room(RoomId), Node(NodeId) }
pub struct Transform { /* se2/se3 */ pub confidence: Confidence }

pub fn room_of(obs: &LinkWindow, home: &Home, cal: &Calibration) -> Scored<RoomId>;
pub fn zone_of(obs: &LinkWindow, home: &Home, cal: &Calibration) -> Scored<Option<ZoneId>>;
pub fn floor_of(obs: &LinkWindow, home: &Home, cal: &Calibration) -> Scored<FloorId>;
pub fn estimate_position(links: &[LinkObservation], home: &Home, cal: &Calibration) -> Scored<PositionEstimate>;

// aquila-calibration
pub fn run(
    source: &impl CsiSource,
    clock: &impl Clock,
    truth: &impl GroundTruth,
    sink: &impl Sink,
    metrics: &impl MetricSink,
    request: CalibrationRequest,
) -> Result<CalibrationReport>;

pub fn parse_artefact(bytes: &[u8]) -> Result<Calibration>;   // for replay/fixtures
pub fn validate_artefact(cal: &Calibration, home: &Home) -> ValidationReport;
```

The wizard drives `run` as a state machine over capture windows; each step is an
independently testable unit.

---

## 7. What global calibration learns

Global (whole-home) in v1: there is no per-room-only calibration. The wizard
sweeps room → floor → cross-floor in one pass.

| Layer | Learned | Used by |
| --- | --- | --- |
| **L1 per-node** | clock offset + skew + fit residual; per-node amplitude/phase gain drift basis; channel/bandwidth/subcarrier config; firmware id | time sync, drift correction |
| **L2 per-node-pair (link)** | empty-home baseline: per-subcarrier amplitude mean & std, noise floor, excess dynamic range, temporal drift basis (PCA), samples/coverage, SNR | baseline subtraction; the perturbation signal everything else consumes |
| **L3 per-zone** | presence & motion thresholds targeting a chosen false-positive rate; zone signature (mean + covariance); contributing links + weights; zone adjacency | attribution, presence/motion detection |
| **L4 per-room / per-floor** | aggregation weights; cross-floor slab attenuation factor; room-to-room discriminability | fusion, floor attribution |
| **L5 global** | geometry (transforms, proximity relations, position priors), quality gates, policy (target FPR, staleness window) | geometry outputs, validity |

Drift is *seeded* at calibration and *tracked continuously* thereafter; a
calibration captures the drift basis, not a frozen correction.

---

## 8. Trigger: the guided wizard

Primary trigger is a **guided wizard served by the HA add-on through sidebar
ingress** (dashboard → core over private HTTP+WS). Secondary triggers: scheduled
re-baseline, invalidation event, first-run onboarding.

Wizard steps (each a capture window with a learned output and a ground-truth
prompt):

1. **Confirm topology** — floors, rooms, zones drawn on the floor plan; nodes
   assigned to rooms. Output: topology + `topology_digest`. No RF needed.
2. **Place nodes** — optional tap-to-place; record mounting and yaw if known.
3. **Empty-home baseline** — home quiet for 5–15 min (configurable, minimum
   enforced). Output: L1 drift seed + L2 baselines. Wizard shows live link
   coverage so the user can see a bad node before wasting the run.
4. **Zone walk-through** — prompt "stand in the kitchen / on the sofa"; capture
   30–60 s per zone. Output: L3 signatures + thresholds.
5. **Room / floor sweep** — brief presence in each room; cross-floor prompts.
   Output: L4 weights + slab attenuation.
6. **Evaluate & publish** — compute quality gates (§11), write the artefact,
   show status. A failing gate blocks publication as `valid` and explains why.

Every wizard step is resumable and produces a partial artefact; the run is
publishable only when all gates for the targeted stages pass.

---

## 9. Artefact and storage

- **Format**: one canonical JSON document per calibration (schema in §13).
  Large baseline arrays are stored as base64 little-endian f32 (deterministic
  bytes); the whole document is content-addressed.
- **Location** (HA add-on persistent storage, never publicly exposed):
  `/data/aquila/homes/<home_id>/calibration/<calibration_id>.json`
  plus `/data/aquila/homes/<home_id>/calibration/index.json` — an append-only
  list with the `current` pointer and each entry's quality summary.
- **Write discipline**: temp file → `fsync` → atomic rename; `index.json`
  updated in the same transaction. A crash never leaves a half-written
  `current`.
- Home Assistant sees only derived state (sensors/attributes: `calibration_status`,
  `calibration_quality`, `calibration_valid`, invalidation reason). The artefact
  itself stays with the add-on; there is no public REST/WS.

---

## 10. Versioning and invalidation

### Identity and versioning

| Field | Meaning |
| --- | --- |
| `schema_version` | artefact format, `aquila.calibration/v1`; major bumps are breaking |
| `calibration_version` | semver of the *learning semantics* (threshold/fusion algorithms) |
| `calibration_id` | `sha256:` over the canonical body — the true version |
| `core_commit` | the core commit that produced/consumed it (commit-keyed ledger) |
| `topology_digest` | hash of floors/rooms/zones/nodes/pairs at capture time |
| `model_versions` | ONNX model ids the calibration assumed |

The core accepts a known `schema_version` major (refuses unknown majors, migrates
minors) and records the mismatch as `stale` rather than crashing.

### Invalidation rules

| Trigger | Scope | Action |
| --- | --- | --- |
| Node added / removed / moved | affected links + zones | invalidate; prompt re-run |
| Room / zone edited, node reassigned | affected room/floor | invalidate; prompt re-run |
| Firmware or channel/bandwidth change | affected node(s) | invalidate links; prompt |
| Model version change | global | mark `stale`; baseline reusable, thresholds re-derived |
| Sustained drift beyond threshold | affected node(s)/links | mark `stale`; online drift correction continues, confidence penalty |
| Baseline age > `stale_after_days` | global | mark `stale`; suggest scheduled re-baseline |
| Quality gate failure | global | `valid = false` with reasons |
| User manual re-run | global | supersede; keep history |

Policy: an invalid or stale calibration is **never silently used**. Sensing
continues in a degraded mode with an explicit confidence penalty, and the
dashboard surfaces `calibration_valid = false` plus the reason(s). The artefact
keeps `supersedes` and `invalidated_by` so the history is auditable.

---

## 11. Measuring calibration quality

Quality is first-class: each run writes a **commit-keyed ledger entry** (via
`MetricSink`) with deterministic, statistical, and gated metrics. Record+diff
only; no CI gating yet.

| Metric | Kind | Meaning | v1 gate |
| --- | --- | --- | --- |
| `link_coverage` | deterministic | fraction of node-pairs with a usable baseline | ≥ 0.9 |
| `baseline_snr_db` | deterministic (min) | worst usable link SNR | ≥ 12 dB |
| `excess_dynamic_range_db` | deterministic | perturbation headroom above noise | ≥ 3 dB |
| `zone_separation` | statistical | inter-zone distribution distance (e.g. Bhattacharyya) | ≥ 1.5 |
| `presence_f1` | statistical (CV) | leave-one-zone-out presence F1 on calibration captures | ≥ baseline (v1 home bar 0.9 at chosen FPR) |
| `drift_residual_ms` | deterministic | post-fit time-sync residual (max) | ≤ 5 ms |
| `range_residual_m` | statistical | RSSI path-loss fit residual per link | reported; drives `range_sigma_m` |
| `within_room_ratio` | derived | fraction of 95% error ellipses inside their room | ≥ 0.7, else positions demoted to advisory |
| `phase_coherence` | structural | constant `none_cross_node` in v1 | informational |

The gates roll up to a status: `good` (all pass with margin), `fair` (all pass,
little margin), `poor` (any gate fails → `valid = false`). The same status is
what the dashboard shows and what the artefact carries. Every metric is
replayable from recorded fixtures, so a calibration quality number is
reproducible, not anecdotal.

---

## 12. Testing strategy (TDD, fakes, fixtures)

- **Fakes**: `FakeCsiSource` replays recorded baseline fixtures; `FakeClock`
  injects offsets/skew; `GroundTruth` supplies wizard prompts and QA labels.
- **Synthetic home generator**: 3 floors, 9+ nodes, parameterised placement and
  path-loss, for geometry and trilateration tests — including the case where the
  error ellipse exceeds the room (asserting demotion to advisory).
- **Property tests**: frame transform round-trip/associativity; confidence
  algebra monotonicity; canonical node-pair id symmetry (`a-b == b-a`).
- **Golden fixtures**: committed artefact examples pin the schema; parse and
  validate are tested against them.
- **State machine tests**: invalidation triggers produce the documented scope and
  never leave a `valid` calibration after a structural change.
- **Recorded, not only synthetic**: real ESP32-S3 baseline captures committed as
  fixtures once available; replay is the default CI path.
- **No committed agent scaffolding** (per brief).

---

## 13. Appendix — calibration artefact schema

Structural schema (`aquila.calibration/v1`). All arrays of physical values are
little-endian f32 base64. `*_sha256` are lowercase hex. `confidence` ∈ [0, 1].

```
Calibration
├── schema_version        string   "aquila.calibration/v1"
├── calibration_id        string   "sha256:<hex>" over body_sha256 content
├── calibration_version   string   semver, learning semantics
├── generated_at          string   RFC 3339 UTC
├── generated_by          object
│     ├── core_commit     string   git commit (commit-keyed ledger)
│     ├── core_version    string   crate version
│     └── run_id          string   unique run id
├── home                  object
│     ├── home_id         string   uuid
│     ├── topology_digest string   "sha256:<hex>"
│     └── topology        object   floors[] rooms[] zones[] nodes[] links[]
├── frames                object
│     ├── world           object   { kind: "enu", origin_ref, units: "metre" }
│     ├── floors[]        object   { floor_id, index, elevation_m }
│     └── rooms[]         object   { room_id, floor_id, transform_world[x,y,yaw],
│                                    polygon_floor_m[][], confidence }
├── nodes[]               object
│     ├── node_id         string
│     ├── floor_id        string
│     ├── room_id         string|null
│     ├── zone_id         string|null
│     ├── placement       object   { position_m[x,y]|null, yaw_rad|null,
│     │                              mounting, source, confidence }
│     ├── clock           object   { offset_s, skew_ppm, fit_residual_ms,
│     │                              estimated_at }
│     ├── firmware        string           esp32s3-csi x.y.z
│     ├── channel         object   { band, primary_channel, bandwidth_mhz,
│     │                              subcarriers }
│     └── gain_drift      object   { amplitude_db_per_h, phase_basis[],
│                                    confidence }
├── links[]               object
│     ├── link_id         string   canonical unordered "a-b"
│     ├── nodes           [string, string]
│     ├── scope           enum     intra_room | inter_room | inter_floor
│     ├── baseline        object
│     │     ├── window    object   { start, end, duration_s, occupancy:"empty" }
│     │     ├── samples   int
│     │     ├── amplitude_mean_db[]   f32 base64, per subcarrier
│     │     ├── amplitude_std_db[]    f32 base64, per subcarrier
│     │     ├── noise_floor_db        f32
│     │     ├── excess_dynamic_range_db f32
│     │     ├── drift_basis           object { pca[] , temporal_basis }
│     │     └── quality   object   { snr_db, coverage }
│     └── proximity       object
│           ├── rssi_mean_dbm      f32
│           ├── range_estimate_m   f32
│           ├── range_sigma_m      f32  (honest per-link bound)
│           ├── relation           enum same_zone_candidate | adjacent | far
│           └── confidence         f32
├── zones[]               object
│     ├── zone_id         string
│     ├── room_id         string
│     ├── label           string
│     ├── polygon_room_m  [][2]     room-frame vertices
│     ├── shape_confidence f32
│     ├── adjacency       [string]
│     └── detection       object
│           ├── presence_threshold_db f32
│           ├── motion_threshold_db   f32
│           ├── target_fpr            f32
│           ├── contributing_links    [link_id]
│           ├── signature             object { feature_mean[], cov[] }
│           └── quality               object { separation, cv_f1 }
├── geometry              object
│     ├── method          enum    proximity_fingerprint | trilateration_secondary
│     ├── resolution      enum    room | zone | sub_room_advisory
│     ├── phase_coherence const   "none_cross_node"
│     ├── global_confidence f32
│     └── trilateration   object
│           ├── enabled           bool
│           ├── nodes_used        [node_id]
│           ├── error_ellipse_95_m [major, minor, orientation]
│           ├── within_room_ratio f32
│           └── confidence        f32
├── quality               object
│     ├── status          enum    good | fair | poor
│     ├── valid           bool
│     ├── gates[]         object  { gate, value|value_min, threshold, pass }
│     └── metrics         object  ledger-shaped, commit-keyed
├── invalidation          object
│     ├── policy_version  string
│     ├── invalidated_by  [reason]
│     ├── stale_after_days int
│     ├── supersedes      string|null  previous calibration_id
│     └── history_pointer string       index.json#<calibration_id>
└── body_sha256           string  sha256 over all fields above
```

### Example (abridged, one node-pair, one zone)

```json
{
  "schema_version": "aquila.calibration/v1",
  "calibration_id": "sha256:9f2c…",
  "calibration_version": "1.0.0",
  "generated_at": "2026-10-13T09:12:44Z",
  "generated_by": { "core_commit": "733d5a7", "core_version": "0.1.0",
                    "run_id": "cal_20261013T0912Z_a1b2c3" },
  "home": { "home_id": "…", "topology_digest": "sha256:ab12…" },
  "frames": {
    "world": { "kind": "enu", "origin_ref": "room:R0:corner", "units": "metre" },
    "floors": [ { "floor_id": "F0", "index": 0, "elevation_m": 0.0 } ],
    "rooms": [ { "room_id": "R2", "floor_id": "F1",
                 "transform_world": [0.0, 3.2, 0.0],
                 "polygon_floor_m": [[0,3.2],[4.2,3.2],[4.2,6.8],[0,6.8]],
                 "confidence": 0.8 } ]
  },
  "nodes": [ {
    "node_id": "n07", "floor_id": "F1", "room_id": "R2", "zone_id": "Z2a",
    "placement": { "position_m": [2.1, 0.4], "yaw_rad": 1.57,
                   "mounting": "wall", "source": "user", "confidence": 0.6 },
    "clock": { "offset_s": 0.014, "skew_ppm": 3.2, "fit_residual_ms": 0.9,
               "estimated_at": "2026-10-13T09:05:00Z" },
    "firmware": "esp32s3-csi 0.4.1",
    "channel": { "band": "2.4ghz", "primary_channel": 6,
                 "bandwidth_mhz": 20, "subcarriers": 64 },
    "gain_drift": { "amplitude_db_per_h": 0.02, "phase_basis": [],
                    "confidence": 0.7 }
  } ],
  "links": [ {
    "link_id": "n07-n12", "nodes": ["n07", "n12"], "scope": "intra_room",
    "baseline": {
      "window": { "start": "2026-10-13T08:55:00Z", "end": "2026-10-13T09:10:00Z",
                  "duration_s": 900, "occupancy": "empty" },
      "samples": 42110,
      "noise_floor_db": -92.1, "excess_dynamic_range_db": 6.4,
      "quality": { "snr_db": 18.3, "coverage": 0.98 }
    },
    "proximity": { "rssi_mean_dbm": -61.2, "range_estimate_m": 4.8,
                   "range_sigma_m": 2.9, "relation": "same_zone_candidate",
                   "confidence": 0.55 }
  } ],
  "zones": [ {
    "zone_id": "Z2a", "room_id": "R2", "label": "sofa",
    "polygon_room_m": [[0.2,0.3],[1.8,0.3],[1.8,1.4],[0.2,1.4]],
    "shape_confidence": 0.5, "adjacency": ["Z2b"],
    "detection": { "presence_threshold_db": 2.7, "motion_threshold_db": 1.4,
                   "target_fpr": 0.03,
                   "contributing_links": ["n07-n12","n07-n15"],
                   "quality": { "separation": 1.9, "cv_f1": 0.93 } }
  } ],
  "geometry": {
    "method": "proximity_fingerprint", "resolution": "room",
    "phase_coherence": "none_cross_node", "global_confidence": 0.58,
    "trilateration": { "enabled": true, "nodes_used": ["n07","n12","n15"],
                       "error_ellipse_95_m": [3.1, 2.2, 0.4],
                       "within_room_ratio": 0.72, "confidence": 0.4 }
  },
  "quality": {
    "status": "fair", "valid": true,
    "gates": [
      { "gate": "link_coverage", "value": 0.98, "threshold": 0.9, "pass": true },
      { "gate": "presence_f1", "value": 0.91, "threshold": 0.90, "pass": true },
      { "gate": "zone_separation", "value": 1.9, "threshold": 1.5, "pass": true },
      { "gate": "drift_residual_ms", "value": 0.9, "threshold": 5.0, "pass": true }
    ]
  },
  "invalidation": { "policy_version": "1", "invalidated_by": [],
                    "stale_after_days": 90, "supersedes": null,
                    "history_pointer": "index.json#sha256:9f2c…" },
  "body_sha256": "…"
}
```

---

## 14. Open questions

- Path-loss exponent: fit globally or per floor? Leaning per floor with a global
  prior, to keep per-home data needs small.
- Zone adjacency: user-authored edges vs inferred from link overlap. v1 user
  first, inference later.
- `stale_after_days`: 90 is a placeholder; needs a real drift study on S3s.
- Whether cross-floor slab attenuation is stable enough to be a threshold, or
  only a soft prior.
