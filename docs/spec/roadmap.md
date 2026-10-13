# Aquila Roadmap

The delivery plan as **tracer-bullet vertical slices**. Every slice cuts the
whole pipeline end to end — a source of CSI, the DSP and inference in between,
an output, and a measurement record — so that each one is demonstrable and
measured from its first commit. We do not build horizontal layers and hope they
meet; each slice is a thin, honest, working path that the next slice thickens.

This document defines *sequence and exit criteria*, not interfaces. Interfaces
are fixed by ADRs as each slice is designed.

## How to read a slice

Each slice has the same shape:

- **Goal** — the single capability it adds, end to end.
- **Entry** — what must already be true (the predecessor's exit criteria).
- **Exercises** — the modules and seams the slice touches. Seams are named in
  the domain glossary (`CsiSource`, `Clock`, `PoseModel`/`VitalsModel`,
  `GroundTruth`, `Sink`, `MetricSink`); a seam earns existence only when a
  second adapter is needed (replay vs live, fake vs ONNX, fixture vs camera).
- **Exit criteria** — the concrete demonstrations and the metrics that must
  move, recorded in the ledger.
- **Ledger rows** — the `MetricSink` records that prove the slice landed.

## Seam and crate map (proposed)

Crate names are `aquila-<english-descriptor>`; the set is provisional and may be
revised by ADR. A crate exists only once a slice needs it.

| Crate | Role | Seam owned |
| --- | --- | --- |
| `aquila-source` | CSI ingestion; recorded-fixture and live-UDP adapters | `CsiSource` |
| `aquila-clock` | software time base and per-node drift estimation | `Clock` |
| `aquila-signal` | conditioning: subcarrier selection, filtering, phase handling | — |
| `aquila-presence` | presence/occupancy and person count | — |
| `aquila-vitals` | breathing and heart rate | `VitalsModel` |
| `aquila-pose` | coarse pose | `PoseModel` |
| `aquila-model` | ONNX inference adapter plus deterministic fakes | `PoseModel`/`VitalsModel` |
| `aquila-calibration` | global whole-home calibration artefacts | — |
| `aquila-fusion` | node-pair and multi-node fusion | — |
| `aquila-home` | home → floor → room → node → node-pair model | — |
| `aquila-groundtruth` | reference labels and fixtures | `GroundTruth` |
| `aquila-sink` | outputs to Home Assistant, files, in-memory | `Sink` |
| `aquila-metrics` | commit-keyed metric ledger | `MetricSink` |
| `aquila-sensing` | pipeline assembly and orchestration | — |
| `aquila-host` | add-on private HTTP + WS, direct node control channel | — |
| `aquila-dashboard` | Rust → WASM management UI | — |

The add-on image and the HACS custom component are packaging artefacts, not
crates: the component is Python/YAML under the add-on's supervision and speaks
only to the host.

## Slice sequence

### Slice 1 — Presence tracer bullet

**Goal.** A recorded CSI fixture flows through signal conditioning to a presence
decision and person count, and writes a measured result — the end-to-end spine
that every later slice widens.

**Entry.** Repo scaffold and CI (tests only, no metric gating); a small recorded
in-domain CSI fixture set with labels.

**Exercises.** `aquila-source` (replay adapter), `aquila-clock` (logical clock),
`aquila-signal`, `aquila-presence`, `aquila-groundtruth` (fixture labels),
`aquila-sink` (in-memory), `aquila-metrics`. First appearance of `CsiSource`,
`Clock`, `GroundTruth`, `Sink`, `MetricSink`.

**Exit criteria.**
- Deterministic replay: identical input produces byte-identical output.
- Presence F1 ≥ 0.90 on the in-domain fixture set.
- A ledger row is written per commit, keyed by commit, and diffable.
- TDD throughout; fake adapters and fixtures, no hardware.

**Ledger rows.** `presence.f1`, `presence.count.mae`, `replay.digest`.

### Slice 2 — Vitals tracer bullet

**Goal.** Breathing and heart rate ride the same pipeline from fixture to
output, measured against a reference trace.

**Entry.** Slice 1 exit.

**Exercises.** `aquila-vitals`, `aquila-model` (`VitalsModel` with fake then
ONNX adapter), `aquila-clock` (window timing).

**Exit criteria.**
- Breathing MAE ≤ 2 brpm against the fixture reference.
- Heart-rate MAE recorded; its v1 bar is fixed from this baseline (honest claim
  — no invented target).
- Vitals are computed on the same frames and clock as presence, not a parallel
  path.

**Ledger rows.** `breathing.mae`, `heart.mae`, `vitals.window.count`.

### Slice 3 — Global whole-home calibration

**Goal.** A calibration pass runs over the whole home and emits a first-class,
commit-keyed calibration artefact that later sensing consumes, removing per-node
and per-session offsets.

**Entry.** Slice 2 exit; fixtures spanning at least two sessions and two nodes.

**Exercises.** `aquila-calibration`, `aquila-home` (nodes as calibration
subjects), `aquila-clock` (per-node drift estimation), `aquila-metrics`.

**Exit criteria.**
- Calibration is a versioned artefact referenced by commit, never an implicit
  global.
- Re-running calibration on the same fixtures reproduces the same artefact
  digest.
- Presence F1 variance across sessions drops versus uncalibrated baseline;
  vital MAE does not regress.
- Global-by-default: whole-home calibration precedes any room-local tuning.

**Ledger rows.** `calibration.digest`, `presence.f1.cross_session`,
`calibration.drift.per_node`.

### Slice 4 — Multi-node single room

**Goal.** Two or more nodes in one room fuse at the measurement level so
spatial diversity raises pose and steadies presence.

**Entry.** Slice 3 exit; fixtures with a genuine node pair in one room.

**Exercises.** `aquila-fusion` (node-pair first), `aquila-pose` (`PoseModel`,
fake then ONNX), `aquila-home` (room boundaries), `aquila-clock` (inter-node
drift).

**Exit criteria.**
- Coarse torso PCK@20 ≥ 65% in-domain for the single-room fixture set.
- Presence F1 holds or improves against the single-node baseline.
- Inter-node drift is estimated and applied, not assumed away.
- Fusion is feature-level, per the settled brief.

**Ledger rows.** `pose.pck20.torso`, `presence.f1.room`, `fusion.node_pair.count`.

### Slice 5 — Floor aggregation

**Goal.** Room-level outputs compose into per-floor presence and room
attribution as the model grows room → floor.

**Entry.** Slice 4 exit; fixtures spanning at least two rooms on one floor.

**Exercises.** `aquila-fusion` (room → floor), `aquila-home` (floor tier),
`aquila-metrics`.

**Exit criteria.**
- Floor-level presence F1 ≥ 0.90.
- Room attribution recorded and baselined; no cross-room leakage regression on
  the presence metric.
- The hierarchy is data, not hard-coded topology.

**Ledger rows.** `presence.f1.floor`, `attribution.accuracy.room`.

### Slice 6 — Cross-floor / whole-home

**Goal.** Whole-home presence, count, and coarse location across floors, with
cross-floor fusion and calibration intact.

**Entry.** Slice 5 exit; fixtures spanning three floors.

**Exercises.** `aquila-fusion` (cross-floor), `aquila-home` (home tier),
`aquila-calibration` (whole-home re-application).

**Exit criteria.**
- Whole-home presence F1 ≥ 0.90 at the reference ~9+ node scale.
- Whole-home person count recorded within the settled band.
- Calibration re-applies across the full topology without per-floor special
  cases.

**Ledger rows.** `presence.f1.home`, `count.mae.home`, `pose.pck20.home`.

### Slice 7 — HACS integration and live nodes

**Goal.** Real ESP32-S3 nodes stream raw CSI over WiFi UDP into the core running
as an HA add-on, and the HACS custom component surfaces sensing outputs as Home
Assistant entities.

**Entry.** Slice 6 exit; reference ESP32-S3 nodes; Linux host only.

**Exercises.** `aquila-source` (live UDP adapter), `aquila-clock` (live drift),
`aquila-host` (private HTTP + WS, direct node control channel), `aquila-sink`
(HA state adapter). The replay adapter and live adapter now both satisfy
`CsiSource`, making the seam real.

**Exit criteria.**
- A real node streams CSI over UDP to the core; presence appears as an HA
  entity without MQTT or ESP-NOW.
- The HACS component installs from the repository and reads only the host; no
  public REST/WS/MCP, no plugins.
- Live drift estimation matches replay-of-capture within tolerance.
- Node control channel drives nodes directly from the add-on.

**Ledger rows.** `live.presence.f1`, `live.clock.drift`, `host.streams.count`.

### Slice 8 — Management dashboard

**Goal.** The add-on serves a Rust → WASM management dashboard over sidebar
ingress: define home/floor/room/node topology, trigger global calibration, and
read the metric ledger.

**Entry.** Slice 7 exit; add-on serving ingress.

**Exercises.** `aquila-dashboard` (WASM build target), `aquila-host` (ingress
and API), `aquila-home` (topology editing), `aquila-calibration` (trigger),
`aquila-metrics` (read view).

**Exit criteria.**
- Dashboard loads via HA sidebar ingress with no separate port.
- The full topology can be authored in the UI and persists through the host.
- Global calibration runs from the UI and its artefact is visible and
  versioned.
- Ledger metrics are readable; dashboard is management-only, not a sensing path.

**Ledger rows.** `dashboard.ingress.ok`, `topology.roundtrip.digest`.

### Slice 9 — Packaging and v1 release

**Goal.** Reproducible add-on image, installable HACS component, documented
install, and the honest-claims statement — everything needed to call v1 shipped.

**Entry.** Slice 8 exit; all prior exit criteria green.

**Exercises.** `aquila-host` (packaging), release tooling, documentation.

**Exit criteria.**
- A clean Home Assistant OS (Linux) install adds the repository, installs the
  add-on and component, and reaches a working sensing pipeline on real nodes.
- The build is reproducible from a tagged commit.
- The v1 metric bars (below) hold on the reference deployment, evidenced by the
  ledger.
- No committed agent scaffolding; `${HOME}` and similar are never baked into
  committed files.

**Ledger rows.** `release.build.digest`, `v1.bars.pass`.

### Slice 10 — Firmware OTA

**Goal.** Node firmware updates over the LAN from the core, signed and verified,
with Home Assistant surfacing and triggering it — no cloud.

**Entry.** Slice 7 exit (live nodes and the control channel).

**Exercises.** `aquila-wire` (OTA control opcodes `0x0E`–`0x14`), `aquila-api`
(OTA endpoints + signed image store), the firmware OTA agent (dual-partition A/B,
Ed25519, rollback), `aquila-ha` (an `update` entity per node). See
`docs/spec/firmware-ota.md`.

**Exit criteria.**
- A signed image installs on a node over the control channel and the node
  confirms it, or rolls back automatically if it fails its health check.
- Each node appears as an `update.aquila_<node>_firmware` entity with installed /
  latest versions and progress.
- No internet and no cloud are involved; the node runs no HTTP server.
- A bad signature is rejected and never activated.

**Ledger rows.** `firmware.ota.result`, `firmware.version.installed`.

## What "v1 shipped" means

v1 is shipped when **all** of the following are true on the reference,
whole-home deployment (Linux host, Home Assistant OS, ~9+ ESP32-S3 nodes over
three floors):

1. Nodes stream raw CSI over WiFi UDP to the add-on; there is no ESP-NOW and no
   MQTT. The control plane is the core's private HTTP + WS and a direct node
   control channel; there is no public REST/WS/MCP and no plugin surface.
2. Home Assistant surfaces presence/occupancy, person count, coarse pose, and
   breathing and heart rate as entities; the HACS component and the sidebar
   ingress dashboard both install from the published repository.
3. The three v1 bars pass **in-domain**, evidenced by commit-keyed ledger rows:
   - in-domain torso PCK@20 ≥ 65%,
   - presence F1 ≥ 0.90,
   - breathing MAE ≤ 2 brpm against the reference.
   Heart-rate MAE is reported with its baseline-fixed bar; claims stay honest
   and are never stronger than the ledger supports.
4. Global whole-home calibration is first-class, versioned by commit, and
   re-applies across the full topology.
5. The pipeline is replay-and-sim first: any ledger row can be reproduced from
   recorded fixtures without hardware.
6. Node firmware updates over the LAN from Home Assistant, signed and verified,
   with automatic rollback — no cloud (`firmware-ota.md`).

Metrics are recorded and diffed across commits; none gate CI yet. v1 does not
claim out-of-domain accuracy, cross-environment generalisation, or any of the
out-of-band tracks below.

## Out-of-band research tracks

These are explicitly **not** v1 and must not block the slice sequence. They may
inform later ADRs but are run separately, with their own honest write-ups:

- **Camera ground truth.** A research-only reference source behind the
  `GroundTruth` seam for offline evaluation; never a runtime dependency.
- **Active sensing.** Any transmit/steering approach; constrained by the
  compliant-802.11 boundary and ACMA LIPD limits.
- **ESP32-as-SDR / phase-coherent array.** Raw I/Q via esp-sdr and coherent
  multi-node arrays; phase-coherent RF reference remains future-only. See
  `docs/research/esp32-as-sdr.md`.

## Ledger and metric discipline

Every slice writes `MetricSink` rows keyed to the commit, classified as
**deterministic** (replay digests, schema, reproducibility), **statistical**
(F1, MAE, PCK, attribution), or **gated** (none in CI yet). The workflow is
record-and-diff: a slice is not done until its rows exist at the target bar and
a diff against the previous commit is available. This is the mechanism that
keeps "measured from commit one" true.
