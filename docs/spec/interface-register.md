# Aquila interface register

The canonical interface register, settled by `docs/adr/0003-interface-reconciliation.md`.
This file is an **index**: it names every crate, its layer, its public interface in
one line, and its dependencies, and it points at the owning spec for detail. It
does not duplicate spec content. Any crate name not listed here is a defect.

- **Contract version:** `1.0` (`CONTRACT_VERSION`), per `data-contracts.md`.
- **Wire version:** `1.0` (`WIRE_MAJOR`/`WIRE_MINOR`), per `data-contracts.md`.
- **DAG data:** `tools/dag.toml`, enforced by `cargo run -p xtask -- dag-check`
  (ADR-0002). The `xtask` tooling crate and `tools/dag.toml` are in-tree and the
  guardrail runs on every check (audit #21/#33 resolved).

Level is the integer ordering the DAG check uses (a crate may depend only on a
strictly lower level); layer is the six-layer name from `architecture.md`.

## Canonical types (one declaration each)

These types are declared exactly once. A second declaration anywhere is a defect.

| Concept | One canonical declaration | Relationship |
| --- | --- | --- |
| CSI frame | `aquila_types::CsiFrame` | The on-air/on-disk contract frame (raw signed int8 I/Q sample `i_q`, interleaved **imaginary-first** `[Q0, I0, …]`; `first_word_invalid` flags an unreliable leading complex sample; `data-contracts.md` §2, ADR-0006; amplitude/phase are derived). |
| CSI conditioning frame | `aquila_signal::ComplexFrame` | `aquila-signal`'s **internal** decode representation (per-subcarrier `Complex32` bins), produced from the contract frame by `aquila_signal::decode_csi`. It is not a contract type and never crosses a crate boundary as a serialised shape. |
| Feature vector | `aquila_types::FeatureVector` | The packed model-input carrier; constructed by `aquila-signal` (`NodeFeatures::feature_vector`) and re-exported by `aquila-inference`. |
| SHA-256 | `aquila_types::sha256` / `to_hex` | The workspace's one hash; every crate calls it rather than carrying its own. |
| Per-node calibration quality | `aquila_types::NodeQuality` | Carried per node in the calibration artefact; fusion's `q_n` (`fusion-sync.md` §2.3). |

## Crate register

| Crate | Level | Layer | Purpose (one line) | Public interface (summary) | Depends on | Owning spec |
| --- | --- | --- | --- | --- | --- | --- |
| `aquila-types` | 0 | L0 vocabulary | Shared alphabet, versioned contracts and the workspace's one SHA-256; no domain behaviour. | Domain newtypes/records (`NodeId`, `RoomId`, `CsiFrame`, `CsiEnvelope`, `SensingFrame`, `Pose`, `VitalsReading`, `Detection`, `Event`, `Session`, `LedgerRecord`, `Calibration`), `FeatureVector`, `ContractVersion`, `Witness`, `NodeQuality`, `Clock` + `CoreTime`/`DeviceTime`, and `sha256`/`to_hex`. | ∅ | `data-contracts.md` |
| `aquila-wire` | 1 | L1 pure transforms | One on-air codec for the CSI data plane and node control plane; generates `node_wire.h`. | `encode_datagram`/`decode_datagram`, control framing; codec constants. | `aquila-types` | `capture-and-firmware.md` §5–6, §9.4 |
| `aquila-signal` | 1 | L1 pure transforms | CSI conditioning: decode, sanitise, filter, features. | `decode_csi` (contract `CsiFrame` → internal `ComplexFrame`), `decode_record`, `normalise`, `unwrap_phase`, `sanitise_phase`, `bandpass`, `resonator`, `motion_features`, `stft`, `bvp_track`, `resample_uniform`, `select_subcarriers`, `process`; owns the internal `ComplexFrame` type and constructs `aquila_types::FeatureVector`. | `aquila-types` | `signal.md` |
| `aquila-geometry` | 1 | L1 pure transforms | Topology → zones, coordinate frames, confidence-scored attribution. | `Topology::load`, `room_of`, `zone_of`, `floor_of`, `position`, `pairs`, `scope_of`, `estimate_position`. | `aquila-types` | `geometry-calibration.md` §1–6 |
| `aquila-metrics` | 1 | L1 pure transforms | Metric definitions and the commit-keyed ledger behind `MetricSink`. | `Metric::id/compute`, `MetricSink::record/flush`, `diff`; `Measurement`. | `aquila-types` | `measurement.md` |
| `aquila-session` | 2 | L2 ingestion & models | Read/write the `.aqs` recorded-fixture container and its manifest. | `SessionReader`/`SessionWriter`, `verify`, manifest parse. | `aquila-types`, `aquila-wire` | `capture-and-firmware.md` §8 |
| `aquila-inference` | 2 | L2 ingestion & models | The `PoseModel`/`VitalsModel` seam and the ONNX adapters. | `PoseModel::infer`, `VitalsModel::infer`, `ModelError`; `PoseRequest`/`PoseOutcome`; re-exports `aquila_types::FeatureVector`. | `aquila-types` | `inference-models.md` |
| `aquila-sources` | 3 | L2 ingestion & models | The core↔node seam: upstream CSI and downstream control, both directions. | `CsiSource::recv/shutdown`, `NodeControl::send`; `ReplaySource`, `UdpLiveSource`, `FanInSource`. | `aquila-types`, `aquila-wire`, `aquila-session` | `capture-and-firmware.md` §3–4, §6 |
| `aquila-simulator` | 4 | L2 ingestion & models | Deterministic, hardware-free CSI synthesis; implements `CsiSource`. | `SimulatorSource`; scene script load; synthesised `GroundTruth`. | `aquila-types`, `aquila-wire`, `aquila-session`, `aquila-sources` | `capture-and-firmware.md` §7 |
| `aquila-sensing` | 5 | L3 core | The single stateful core: time, calibration, windows, fusion, escalation. | `Sensing::new/step/calibrate/sink`; owns the `Clock` and `Sink` seams; internal `time`/`calibration`/`window`/`fusion`/`escalation` modules. | `aquila-types`, `aquila-geometry`, `aquila-signal`, `aquila-sources`, `aquila-inference`, `aquila-metrics` | `architecture.md` §4, `geometry-calibration.md`, `fusion-sync.md` |
| `aquila-evaluation` | 6 | L4 edges | The `GroundTruth` seam, replay/sim harness, and scoring. | `GroundTruth::label/coverage/kind/provenance`; `Harness::run`; `CollectorSink`. | `aquila-types`, `aquila-sources`, `aquila-sensing`, `aquila-metrics`, `aquila-simulator` | `evaluation.md` |
| `aquila-ha` | 6 | L4 edges | Map observations to Home Assistant entities; transports nothing. | `HaSink: Sink`; `EntityModel`. | `aquila-types`, `aquila-geometry`, `aquila-sensing` | `home-assistant.md` |
| `aquila-api` | 7 | L4 edges | The private loopback HTTP+WS control plane and node-command relay. | `Server::bind/handle`; `Handle::publish`; `ChannelSink`. | `aquila-types`, `aquila-geometry`, `aquila-sensing`, `aquila-sources`, `aquila-ha` | `core-api.md` |
| `aquila-dashboard` | 8 | L5 end adapters | Rust→WASM management UI served via sidebar ingress. | WASM entry point; Leptos SPA; client of `aquila-api`. | `aquila-types`, `aquila-api` | `dashboard.md` |

## Composition root and tooling (unlayered, exempt)

| Crate | Kind | Purpose | Depends on | Owning spec |
| --- | --- | --- | --- | --- |
| `aquila-addon` | binary | Composition root: wires adapters, serves dashboard + ingress; no domain logic. | any module crate | `architecture.md` §9, `home-assistant.md` §3 |
| `aquila-capture` | binary (dev) | `record`/`synth`/`verify`/`provision` a Session; regenerates `node_wire.h`. | `aquila-types`, `aquila-session`, `aquila-sources`, `aquila-simulator`, `aquila-wire` | `capture-and-firmware.md` §8.3, §9.4 |
| `aquila-testkit` | lib (dev-only) | Fakes, in-memory adapters, a deterministic generator, a virtual live node, fixtures. | any module crate, via `[dev-dependencies]` only | `testing.md` §3 |
| `xtask` | binary (tooling) | `dag-check`, `check-scaffolding`, `coverage-floor`, `claims-verify`. | any crate; never a product dependency | `repository-tooling.md` §5 |

## Seams and their adapters

Six seams (ADR-0001 §10). Each has two or more real adapters.

| Seam | Trait home | Adapter A (production) | Adapter B (test/replay) | Other adapters |
| --- | --- | --- | --- | --- |
| `CsiSource` | `aquila-sources` | `UdpLiveSource` | `ReplaySource` | `SimulatorSource` (`aquila-simulator`), `FanInSource` |
| `Clock` | trait in `aquila-types`; seam owned by `aquila-sensing` | `SystemClock` | `ReplayClock` | — |
| `PoseModel` / `VitalsModel` | `aquila-inference` | `OnnxPoseModel`, `OnnxVitalsModel` | `FakePoseModel`, `FakeVitalsModel` (`aquila-testkit`) | — |
| `GroundTruth` | `aquila-evaluation` | `FixtureGroundTruth` | `CameraGroundTruth` (research-only) | `MmFiLabelGroundTruth`, `ReferenceVitalsGroundTruth` |
| `Sink` | `aquila-sensing` | `ChannelSink` (`aquila-api`), `HaSink` (`aquila-ha`) | `CollectorSink` (`aquila-evaluation`) | — |
| `MetricSink` | `aquila-metrics` | `JsonLedgerSink` | `InMemoryMetricSink` | `TeeSink`, `CiCacheSink` |

`NodeControl` is **not** a seam: it is the outbound half of the `CsiSource`
protocol in the same crate with the same adapters (ADR-0001 §3).

## Wire ports

| Plane | Transport | Port | Owner | Owning spec |
| --- | --- | --- | --- | --- |
| CSI data plane (node → core) | UDP unicast, one CSI frame per datagram | `9100/udp` | core | `capture-and-firmware.md` §5 |
| Node control plane (core ↔ node) | UDP request/response | `9101/udp` | core | `capture-and-firmware.md` §6 |
| Private HTTP + WebSocket + HA ingress | HTTP/1.1 (+ WS, RFC 6455), loopback/ingress only | `8099/tcp` | add-on | `core-api.md`, `home-assistant.md` §3.1 |

`6969/udp` (`home-assistant.md`), `8765` (`core-api.md` §2) and `8080`
(`dashboard.md`) are superseded by this register (ADR-0003 §7–8).

## Firmware OTA

Node firmware update rides the node control plane (`9101/udp`) via opcodes
`0x0E`–`0x14` (`docs/spec/firmware-ota.md` §5) — no new port, no node HTTP
server. The core owns a signed image store under `/data` and exposes
`/api/v1/firmware*` (`home-assistant.md` §5); the HACS component adds one `update`
entity per node. Ed25519-signed images, dual-partition A/B, health check and
rollback. No new crate.

## Dependency edges (authoritative)

```
aquila-types      -> ∅
aquila-wire       -> aquila-types
aquila-signal     -> aquila-types
aquila-geometry   -> aquila-types
aquila-metrics    -> aquila-types
aquila-session    -> aquila-types, aquila-wire
aquila-inference  -> aquila-types
aquila-sources    -> aquila-types, aquila-wire, aquila-session
aquila-simulator  -> aquila-types, aquila-wire, aquila-session, aquila-sources
aquila-sensing    -> aquila-types, aquila-geometry, aquila-signal,
                     aquila-sources, aquila-inference, aquila-metrics
                     (aquila-metrics added at implementation: the core records
                      per-step Measurement rows through the MetricSink seam.)
aquila-evaluation -> aquila-types, aquila-sources, aquila-sensing,
                     aquila-metrics, aquila-simulator
aquila-ha         -> aquila-types, aquila-geometry, aquila-sensing
aquila-api        -> aquila-types, aquila-geometry, aquila-sensing,
                     aquila-sources, aquila-ha
aquila-dashboard  -> aquila-types, aquila-api
```

`aquila-addon`, `aquila-capture`, `aquila-testkit` and `xtask` are exempt from
the level rule; `aquila-testkit` may appear only under `[dev-dependencies]`.

## References

- `docs/adr/0001-module-architecture.md`, `docs/adr/0002-toolchain-and-repo.md`,
  `docs/adr/0003-interface-reconciliation.md`.
- `docs/spec/architecture.md`, `docs/spec/data-contracts.md`,
  `docs/spec/repository-tooling.md`.
- `docs/reference/sources.json` for every external claim (e.g. `esp-csi`, `ort`,
  `leptos`, `hacs`, `ha-addons`).
