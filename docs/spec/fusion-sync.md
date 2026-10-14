# Spec — Fusion & Time Sync

Status: draft v1. Owner: fusion & time-sync author.
Scope: the `Clock` seam, software time synchronisation, and multi-node fusion at
the feature/detection level. This document does **not** cover per-node CSI DSP,
the model zoo, global calibration mechanics, or the HA surface; it consumes their
outputs through the interfaces named below.

Vocabulary follows the Aquila brief. Prose is Australian English.

Related crates (proposed, in DAG order — no cycles):

```
aquila-core        # shared value types: CoreTime, DeviceTime, NodeId, RoomId, FloorId, Topology
aquila-timesync    # offset + drift estimation, device→core time correction  (depends: aquila-core)
aquila-fusion      # feature/detection fusion, spatial units, conflict resolution (depends: aquila-core, aquila-timesync)
```

`aquila-timesync` MUST NOT depend on `aquila-fusion`; fusion consumes a
`NodeClockModel` but never mutates one.

---

## 1. The `Clock` seam and software time synchronisation

### 1.1 Two clocks, one truth

- **Core clock** — a monotonic host clock on the add-on, nanoseconds since core
  boot. This is the single reference; all inference, windows and ledgers are
  expressed in it.
- **Device clock** — each node's `esp_timer` monotonic counter, nanoseconds since
  node boot. Cheap, uncorrected, and drifting relative to the core.

Nodes are dumb: they stamp every CSI frame with their own device time and never
adjust it. The core owns correction. There is no GPS, no PTP, no NTP; the only
channels are the core's private HTTP+WS control plane and the direct node control
channel. WiFi hardware here exposes no PHY TX/RX timestamp, so all measurement is
software-timestamped at the MAC/application boundary — hence millisecond, not
nanosecond, accuracy (see §1.5).

The `Clock` seam is the core's injectable source of "now":

```rust
/// Monotonic core time, nanoseconds since core boot.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct CoreTime(pub u64);

/// The injectable core time source. Real adapter wraps a monotonic host clock;
/// fake adapter (ManualClock) advances on command for deterministic tests.
pub trait Clock: Send + Sync {
    fn now(&self) -> CoreTime;
}
```

The real adapter MUST be monotonic and MUST NOT jump (no wall-clock, no NTP
step). Persisted states carry a boot id so a core restart is detected rather
than misread as a backward step.

### 1.2 Offset estimation (NTP-style round trip)

Offset is estimated with a four-timestamp exchange over the control channel,
piggybacked on the existing sync cadence (default one round per node every
**2 s**, §1.6). One exchange:

```
core  -- t1 (CoreTime, send) -->            node
                                            t2 (DeviceTime, receive)
                                            t3 (DeviceTime, send)
core  <-- t2, t3 ---------------------      node
        t4 (CoreTime, receive)
```

For round *i* on node *n*:

```
offset_n,i = ((t2 - t1) + (t3 - t4)) / 2      // device-time minus core-time at midpoint
rtt_n,i    = (t4 - t1) - (t3 - t2)            // round-trip, node processing removed
anchor_n,i = (t1 + t4) / 2                    // core-time midpoint for this sample
```

`rtt` is a direct measure of the one-way uncertainty: the residual offset error
of a single sample is bounded by `rtt/2` (asymmetry is unobservable in software).
Samples are therefore gated before use:

- **RTT gate**: reject any round with `rtt > rtt_reject` (default **20 ms**).
- **RTT filter**: within a window keep the lowest-RTT fraction (default keep the
  best **50 %**, i.e. discard the noisy upper half).
- **Outlier gate**: reject offset samples more than `k·MAD` (default `k = 4`)
  from the window median.

Discarded rounds are counted and surfaced as node `SyncHealth`, never silently
dropped. `SyncHealth` feeds the fusion weight (§2.3).

### 1.3 Linear drift fit

Crystal frequency error is modelled as an affine map, fitted per node over a
sliding window:

```
N(t) ≈ (1 + ε_n) · t + φ_n            // ε_n: relative rate error; φ_n: offset at epoch
```

Equivalently, correcting a device stamp to core time:

```
core(t_dev) = (t_dev - φ_n) / (1 + ε_n)
```

The model is a **weighted least-squares fit** of the gated `(anchor_i, offset_i)`
samples:

```
offset(C) = φ_n + ε_n · C            // regress offset against core anchor
```

- Window: default **64 accepted samples** (~2 min at 2 s cadence), FIFO.
- Weights: `1 / max(rtt_i, rtt_floor)`  — tighter round trips dominate.
- Guards: accept the fit only if `|ε_n| ≤ ε_max` (default **±200 ppm**) and
  `≥ 8` accepted samples remain; otherwise the node is `Marginal` at best.
- Quality: `residual_rms` of the fit sets `NodeClockQuality`
  (`Good` / `Marginal` / `Unusable`) via fixed thresholds (§1.5).

Refit cadence: every accepted round after warm-up (rolling), so the model tracks
slow temperature drift without discontinuity. `slope_ppm` and `phi_ns` are
published with the `epoch` at which `φ_n` was defined and the `fitted_at` time,
so a correction is reproducible from a persisted model alone.

### 1.4 Applying the correction

The core corrects at ingest, once per CSI frame:

1. Look up `NodeClockModel` for the frame's node.
2. If `Unusable` or absent → the frame's timing is quarantined: it MAY still feed
   presence (§2.3) at reduced weight but MUST NOT feed vitals or pose timing.
3. Compute `CorrectedTime { core, uncertainty_ns }`, where `uncertainty_ns` is
   propagated from `residual_rms` plus the extrapolation term (slope error ×
   elapsed since `fitted_at`).
4. **Sanity gate**: compare `corrected(core)` with the frame's core arrival time
   `t_arr`. If `|c(core) − t_arr| > sync_sanity` (default **25 ms**), flag the
   node `Marginal`, emit a `SyncAnomaly`, and downgrade weight for that window.

Provenance is preserved: every ingested frame keeps `(node, raw device ts, raw
arrival ts, corrected core ts, clock quality)` in the record so any downstream
result can be traced to the timestamps and the model revision that produced it.

### 1.5 Accuracy budget

Nominal targets, at the stated cadence after warm-up (`≥ 32` accepted samples):

| Quantity                                   | Goal    | Gate (fail closed) |
| ------------------------------------------ | ------- | ------------------ |
| Single-node offset error (RMS)             | ≤ 2 ms  | ≤ 10 ms            |
| Cross-node relative alignment (RMS)        | ≤ 3 ms  | ≤ 5 ms (p99)       |
| Cross-node relative alignment (p99)        | ≤ 5 ms  | ≤ 10 ms            |
| Drift slope error                          | ≤ 2 ppm | ≤ 10 ppm           |
| Extrapolation error over a 10 s gap        | ≤ 40 µs | ≤ 200 µs           |
| Fit residual RMS (Good)                    | ≤ 500 µs| ≤ 2 ms (Marginal)  |

Rationale: breathing is ~0.1–0.5 Hz and heart rate ~0.8–2 Hz; a 5 ms relative
skew is < 4° of phase at heart-rate peak and irrelevant to spectra. Pose frames
at 20 Hz (50 ms) tolerate 10 ms skew. The dominant error term is offset jitter
bounded by LAN RTT, not drift. **Sub-millisecond or sub-nanosecond alignment is
explicitly out of budget** — it requires a hardware RF reference (§3).

### 1.6 Sync cadence and failure behaviour

- Default **1 round / node / 2 s**, jittered to avoid lockstep bursts.
- Per-node back-off on repeated RTT rejection (2 s → 30 s), reset on success.
- Cold start: a node is `Unusable` until `≥ 8` accepted samples; frames before
  that contribute to presence only.
- On persistent failure the last `Good` model is used with a growing
  `uncertainty_ns`; after `clock_hold_max` (default **120 s**) the node becomes
  `Unusable` and its timing-sensitive output is suppressed.

### 1.7 Time-sync interface

```rust
/// Node-local device time, nanoseconds from esp_timer.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct DeviceTime(pub u64);

/// One NTP-style round trip captured over the control channel.
#[derive(Clone, Copy, Debug)]
pub struct SyncRound {
    pub node: NodeId,
    pub core_send: CoreTime,     // t1
    pub device_recv: DeviceTime, // t2
    pub device_send: DeviceTime, // t3
    pub core_recv: CoreTime,     // t4
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeClockQuality { Good, Marginal, Unusable }

#[derive(Clone, Copy, Debug)]
pub struct CorrectedTime {
    pub core: CoreTime,
    pub uncertainty_ns: u64,
}

/// Immutable per-node clock model. Reproducible from this struct alone.
#[derive(Clone, Debug)]
pub struct NodeClockModel {
    pub node: NodeId,
    pub epoch: CoreTime,          // time at which phi_ns is defined
    pub fitted_at: CoreTime,
    pub slope_ppm: f64,           // epsilon_n * 1e6
    pub phi_ns: i64,              // offset at epoch, device - core
    pub rtt_median_ns: u64,
    pub residual_rms_ns: u64,
    pub accepted_rounds: u32,
    pub quality: NodeClockQuality,
}

impl NodeClockModel {
    /// Deterministic correction: t_core = (t_dev - phi)/ (1 + slope).
    pub fn correct(&self, t: DeviceTime) -> CorrectedTime;
    pub fn quality_at(&self, now: CoreTime) -> NodeClockQuality; // degrades with age
}

#[derive(Clone, Debug)]
pub struct SyncConfig {
    pub rtt_reject_ns: u64,        // 20 ms
    pub rtt_keep_fraction: f64,    // 0.5
    pub mad_k: f64,                // 4.0
    pub window: usize,             // 64
    pub min_samples: usize,        // 8
    pub eps_max_ppm: f64,          // 200
    pub clock_hold_max_ns: u64,    // 120 s
    pub sync_sanity_ns: u64,       // 25 ms
}

#[derive(Clone, Debug)]
pub struct SyncAnomaly { pub node: NodeId, pub at: CoreTime, pub kind: SyncAnomalyKind }

/// Deep module: ingests rounds over time, owns per-node models.
/// Pure with respect to the sequence of observed rounds.
pub struct ClockSynchroniser { /* private */ }

impl ClockSynchroniser {
    pub fn new(cfg: SyncConfig, clock: Arc<dyn Clock>) -> Self;

    /// Feed one round trip; returns the updated model for that node.
    /// Must be deterministic: the result depends only on the rounds seen.
    pub fn observe(&mut self, round: SyncRound) -> Result<&NodeClockModel, SyncError>;

    pub fn model(&self, node: NodeId) -> Option<&NodeClockModel>;
    pub fn health(&self, node: NodeId) -> SyncHealth;

    /// Persistable snapshot for replay and restart; carries the core boot id.
    pub fn snapshot(&self) -> SyncSnapshot;
    pub fn restore(snapshot: SyncSnapshot, cfg: SyncConfig, clock: Arc<dyn Clock>) -> Self;
}
```

`ClockSynchroniser` drives round trips through a small, separate transport
port (the control channel); that port is a fake in tests, so sync is fully
exercised without hardware.

---

## 2. Multi-node fusion (feature / detection level)

Fusion combines **per-node features and detections only**. There is no
cross-node raw-phase combination (§3): each node is a *unary* sensor over its own
coverage volume. Fusion is a pure function of `(Topology, CalibrationQuality[],
NodeEvidence[], FusionConfig, prev state)` and is deterministic (§2.7).

### 2.1 Spatial units and evidence contract

Topology is explicit from day one: `Home → Floor → Room → Node → (NodePair)`.
Each node belongs to exactly one room; each room to exactly one floor. Node pairs
exist in the model but are **not** used for coherent fusion in v1.

A node's calibrated pipeline emits one `NodeEvidence` per aligned window (default
1 s hop):

```rust
pub struct NodeEvidence {
    pub node: NodeId,
    pub window: TimeWindow,             // core-time aligned, half-open
    pub presence_logit: f64,            // log-odds of presence in this node's volume
    pub count_pmf: CountPmf,            // P(n people) in this node's volume, n in 0..=MAX
    pub vitals: Option<NodeVitals>,     // breathing/heart rate + quality
    pub pose: Option<NodePose>,         // coarse pose relative to this node
    pub calibration_quality: Quality,   // q_n in [0,1] from global calibration
    pub health: NodeHealth,             // packet loss, RSSI, SyncHealth
    pub coherence: Coherence,           // Uncoherent in v1
}
```

`Coherence` is the seam that a future phase-coherent array fills (§3.2). In v1
it is always `Coherence::Uncoherent`, and fusion MUST reject any element claiming
coherence it has no combiner for.

### 2.2 Combining per-node detections

**Presence** — a binary decision over a volume — uses **log-odds pooling** (a
Bayesian product of independent evidence):

```
L_unit = L_prior(unit) + Σ_n  w_n · L_n        // L_n = presence_logit, w_n in [0,1]
p_unit = sigmoid(L_unit)
```

- Each node's contribution is **clamped**: `|w_n · L_n| ≤ L_max` (default 4.0,
  ≈ 0.98 confidence) so no single node can force occupancy.
- Weights `w_n` come from §2.3.
- Log-odds pooling is commutative and associative, so the result is independent
  of node ordering (an invariant, §2.7).
- Absence is **not** the default: the prior encodes "unknown" (`L_prior` for an
  uncalibrated/empty unit is 0 → `p = 0.5` → `Occupancy::Unknown`), never
  `Absent`.

**Count** — a bounded quantity — uses a **weighted arithmetic mixture** of node
PMFs, which avoids the over-confidence of pooling counts as independent
decisions:

```
P_room(n) = Σ_i  ŵ_i · P_i(n) / Σ_i ŵ_i        // renormalised mixture
```

`CountEstimate { map: argmax P, confidence: P(map), pmf: P }`. Two nodes seeing
the *same* person in the same room must not double-count, so nodes **within a
room are mixed, not summed**. Summing is reserved for crossing spatial
boundaries (§2.5), where a convolution is used and its independence assumption
is flagged (§2.6).

**Vitals** — fused per room by **quality-weighted selection**, not coherent
phase combination: take the highest-quality estimate whose confidence exceeds a
threshold; if the top two agree within tolerance, report the weight-averaged
rate; if they disagree beyond tolerance, report `FusedVitals::Unresolved` listing
both, and emit a `Conflict`. Vitals never fabricate a rate from absent evidence.

**Pose** — reported per node as coarse pose within that node's view. No
cross-node 3D triangulation is attempted (§3). At room level, fusion reports the
highest-confidence node pose and records that other nodes disagree or are silent;
a room pose derived from multiple *uncoherent* views is out of scope for v1.

### 2.3 Weighting by calibration quality

Each calibrated node carries its own quality `NodeQuality` (`aquila_types`,
a `[0,1]` score), stored in the calibration artefact as `nodes[].quality`
(`geometry-calibration.md` §13) and derived from that node's empty-home baseline
sampling and clock fit. Fusion derives the weight:

```
w_n = clip(q_n, 0, 1) · health_factor(health_n, SyncHealth_n)     // in [0,1]
```

- `q_n` is **this node's** calibration-quality score (`nodes[].quality`) — the
  calibration author's per-node contract, not a whole-home proxy.
- `health_factor` is a bounded penalty product over recent packet loss, stale
  model age, and clock quality: `Good → 1.0`, `Marginal → ≤ 0.6`,
  `Unusable → presence-only, ≤ 0.3`.
- A node with `q_n = 0` contributes **nothing** (invariant, §2.7).
- Weights are **not** normalised for presence (they are evidence precisions);
  they **are** normalised for the count mixture.
- Weight derivation is pure and deterministic given the per-node `NodeQuality`
  and `NodeHealth`.

Uncalibrated nodes MAY contribute presence; they MUST NOT contribute count,
vitals, or pose.

### 2.4 Conflict resolution

Disagreement is expected — nodes see different volumes. It is resolved by
**reporting, not by averaging away**:

1. **Spatial disagreement** — if a unit's room-level presence mass is spread
   across more than one room beyond `room_gap` (default max−second max < 0.15),
   emit `Conflict::AmbiguousRooms` with each room's confidence and set the
   affected floor/home occupancy to `Unknown` unless the home-level pooled
   probability clears `p_hi` (default 0.8); do **not** report a count for the
   ambiguous unit.
2. **Count inconsistency** — if the convolved unit count implies more occupants
   than a configured home prior (or than another calibrated floor strongly
   supports), emit `Conflict::CountInconsistent`. The conflicting unit's count
   degrades to `Unknown` with the candidate counts listed, rather than silently
   trusting the sum.
3. **Vitals disagreement** — as §2.2: reject the lower-quality estimate; if equal
   quality, `Unresolved` plus a conflict.
4. **Temporal hysteresis** — unit state changes require agreement on the new
   state for `T_hold` (default 3 windows). Implemented as a deterministic reducer
   that takes the previous `FusedState` as explicit input; it is idempotent on
   repeated identical windows.
5. **Occupancy vs count** — a unit reported `Absent` forces count `0`; a unit
   `Unknown` never yields a count. `Absent` requires pooled `p < p_lo`
   (default 0.2) with hysteresis, never mere silence.

Every conflict carries the contributing node ids (canonical-sorted), per-node
confidences, and the window — enough to reproduce and diff.

### 2.5 Room / floor / home derivation

Staged, as per the brief — a unit only graduates when its calibration passes:

1. **Room occupancy**: pooled log-odds over the room's calibrated nodes (§2.2).
2. **Room count**: weighted mixture of the room's node PMFs → MAP + confidence.
3. **Floor occupancy**: soft-OR across the floor's rooms:
   `p_floor = 1 − Π_rooms (1 − p_room)`.
4. **Floor count**: **convolution** of the floor's room count PMFs
   (`P_floor = P_r1 ⊛ P_r2 ⊛ …`), then MAP + confidence. A room with `Unknown`
   count contributes a broad uninformative factor and forces the floor count to
   `Unknown` with a coverage note.
5. **Home**: soft-OR over floors for occupancy; convolution over floors for count.

Staging rules:

- Room → floor → cross-floor graduation is gated by calibration quality; an
  ungated unit reports presence only. **Cross-floor fusion is the last step and
  is off until per-floor calibration passes.**
- Unknown coverage is first-class: a unit with zero calibrated nodes reports
  `Occupancy::Unknown` and `CountEstimate::Unknown` with `Coverage::Uncalibrated`,
  never `Absent`/`0`. Absence and unknown are never conflated.

### 2.6 Honesty notes on the model

- Convolution across rooms assumes room counts are independent. A single occupant
  seen by two rooms (e.g. in a doorway) can therefore over-count. This is an
  approximation and is mitigated by conflict detection (§2.4.2), not eliminated.
- Coarse pose from uncoherent views is view-relative, not world-frame.
- All rates/counts are estimates with published uncertainty; no claim is made
  where evidence is absent or a unit is uncalibrated.

### 2.7 Determinism invariants (required tests)

Fusion and time sync MUST be deterministic. These are unit/property/integration
tests with fake adapters and recorded fixtures — no CI gating yet (§ brief:
record + diff).

Time sync:

1. **Round-trip inverse**: injecting a node with known `(ε, φ)` and noiseless
   rounds recovers `slope_ppm`, `phi_ns` within tolerance; `correct` inverts the
   injection.
2. **RTT gate**: rounds above `rtt_reject` never enter the fit; a high-RTT
   window leaves quality no better than `Marginal`.
3. **Offset sign**: a node clock ahead of core yields positive offset and a
   correction that moves its stamps earlier.
4. **Monotonic correction**: corrected times are non-decreasing for a
   non-decreasing device-time sequence (within the uncertainty budget).
5. **Extrapolation bound**: over a fixed gap, corrected error stays within the
   budget given the fitted slope error.
6. **Snapshot round-trip**: `restore(snapshot)` reproduces bit-identical models
   and corrections.

Fusion:

7. **Permutation invariance**: `fuse` returns byte-identical `FusedState` for any
   permutation of the same `NodeEvidence` set.
8. **Duplicate idempotence**: duplicate `(node, seq)` frames are deduplicated;
   fusing a window twice equals fusing it once.
9. **Zero-weight inertness**: a node with `q_n = 0` changes nothing.
10. **Empty input**: no nodes → `Unknown` occupancy, `Unknown` count, never a
    fabricated presence.
11. **Monotonicity**: adding a node with positive logit and non-negative weight
    cannot decrease pooled presence probability.
12. **Count conservation**: home count PMF equals the convolution of floor PMFs;
    with single-room evidence the home MAP equals that room's MAP.
13. **Uncalibrated restraint**: an uncalibrated node contributes presence but
    never count/vitals/pose.
14. **Conflict surfaced**: constructed disagreement raises the expected
    `Conflict` variant with the expected contributors; agreement raises none.
15. **Golden fixtures**: recorded multi-node capture → a content hash of
    `FusedState`; record + diff across runs and machines (no wall clock, no
    `HashMap` iteration — `BTreeMap`/canonical-sorted vectors only).

---

## 3. What is NOT possible without a shared RF reference

v1 has no phase-coherent RF reference between nodes. Software time sync reaches
millisecond accuracy (§1.5); it cannot reach the sub-nanosecond scale that RF
phase implies. Consequently, the following are **explicitly out of scope** and
MUST NOT be claimed or approximated as if coherent:

- **Distributed antenna array** behaviours: cross-node angle-of-arrival (AoA),
  phase-difference-of-arrival, joint MUSIC/ESPRIT/subspace estimation,
  distributed beamforming, synthetic aperture.
- **Cross-node ranging / ToF**: no phase- or delay-based time-of-flight between
  nodes; no hyperbolic/RSSI-independent localisation.
- **Coherent complex combining**: nodes' raw CSI phases cannot be added; only
  magnitudes/derived features can be combined.
- **Multi-view 3D pose**: cross-node pose triangulation is impossible; pose stays
  per-node and view-relative.
- **Cross-node vitals phase fusion**: per-node breathing/heart rates can be
  selected and averaged, but their phase/respiration waveforms cannot be
  combined coherently across nodes.
- **Sub-ms cross-node alignment**: unavailable without a hardware reference. The
  node's own CSI phase is also uncalibrated per subcarrier (CFO/SFO, front-end
  phase), so even a fortuitous time alignment would not yield coherent phase
  without a known pilot/reference.

The home sensing claim in v1 rests on **feature-level fusion of unary sensors**,
not on spatial coherence: presence and count from overlapping room coverage,
coarse per-node pose, and rate-level vitals.

### 3.1 Non-goals restated

No localisation beyond the room granularity of a node's own coverage volume; no
joint spatial nulling for counting; no world-frame pose; no array gain.

### 3.2 Interface where a phase-coherent array would plug in

Two seams are reserved. A *coherent* deployment supplies a `PhaseReference`
(shared clock/RF reference) and groups nodes into a coherent array; fusion then
dispatches coherence-aware combining instead of feature pooling. Nothing in v1
implements these; they exist so the signature does not have to change later.

```rust
/// A shared reference enabling phase-coherent combination across a node group.
/// v1 has no implementation.
pub trait PhaseReference: Send + Sync {
    /// Sub-nanosecond error of the reference (contrast §1.5's millisecond budget).
    fn reference_uncertainty_ns(&self) -> u64;
    fn group(&self) -> ArrayId;
}

#[derive(Clone, Copy, Debug)]
pub enum Coherence {
    Uncoherent,                                   // v1: every node
    Coherent { array: ArrayId, via: ReferenceId }, // future phase-coherent array
}

/// Coherence-aware combiner. v1 registers only the Uncoherent path (§2).
/// A coherent combiner may produce AoA / joint localisation / coherent vitals.
pub trait SpatialCombiner {
    fn coherence(&self) -> Coherence;
    fn combine(&self, evidence: &[NodeEvidence]) -> SpatialEstimate;
}

pub trait Fusion {
    fn fuse(&self, topology: &Topology, inputs: &[NodeEvidence], prev: Option<&FusedState>) -> FusedState;
}
```

Rules for the future plug-in:

- Fusion dispatches on `NodeEvidence.coherence` to a registered
  `SpatialCombiner`; the `Uncoherent` combiner is the v1 implementation and MUST
  remain supported for nodes outside any coherent group.
- A node in a coherent group may additionally contribute a `SpatialEstimate`
  (AoA/localisation/coherent vitals) that replaces the room-level mixture for
  that group; uncoherent nodes are pooled as today.
- The `Clock` seam stays unchanged; `PhaseReference` is an additional capability,
  not a replacement, and its tighter uncertainty only affects timing-sensitive
  branches (it never relaxes the §2.7 determinism invariants).
- Determinism invariants (§2.7) MUST hold on both paths, including
  permutation invariance and no fabricated output when a coherent group is
  incomplete.

---

## Appendix A — defaults

| Constant         | Default | Meaning                                   |
| ---------------- | ------- | ----------------------------------------- |
| sync cadence     | 2 s     | one round / node                          |
| `rtt_reject_ns`  | 20 ms   | gate                                      |
| `rtt_keep_frac`  | 0.5     | keep lowest-RTT half                      |
| `mad_k`          | 4.0     | offset outlier gate                       |
| `window`         | 64      | accepted rounds in drift fit              |
| `min_samples`    | 8       | warm-up before `Good`                     |
| `eps_max_ppm`    | 200     | slope guard                               |
| `sync_sanity_ns` | 25 ms   | corrected vs arrival mismatch             |
| `clock_hold_max` | 120 s   | stale-model suppression                   |
| fusion hop       | 1 s     | evidence window                           |
| `L_max`          | 4.0     | per-node logit clamp                      |
| `room_gap`       | 0.15    | ambiguity threshold (max − second)        |
| `p_lo` / `p_hi`  | 0.2/0.8 | absent / present thresholds               |
| `T_hold`         | 3       | windows to confirm a state change         |

All constants live in `SyncConfig` / `FusionConfig`; none are hidden as literals.
