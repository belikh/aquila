# Aquila — Module Architecture

Status: proposed (see `docs/adr/0001-module-architecture.md`)
Scope: the Rust workspace under `crates/`. The HA custom component (Python) and
the add-on packaging sit outside this crate graph and only meet it at the seams
named in §7 and §8.

This document is settled by the Aquila brief. It does not restate the brief; it
turns it into a crate/module decomposition, a dependency DAG, and a small set of
justified seams.

## 1. Vocabulary

We design **deep modules**: a large body of behaviour behind a small interface,
placed at a clean seam, testable through that interface.

- **Module** — anything with an interface and an implementation; here, a crate.
- **Interface** — everything a caller must know: signatures, invariants,
  ordering, error modes, configuration, performance.
- **Implementation** — the body of code inside a module.
- **Seam** — a place behaviour can be altered without editing in that place; the
  location at which an interface lives.
- **Adapter** — a concrete thing that satisfies an interface at a seam.
- **Depth** — leverage per unit of interface a caller (or test) must learn.
- **Locality** — change, bugs, and verification concentrated in one place.

Two rules drive the decomposition:

1. **One adapter means a hypothetical seam; two adapters means a real one.** We
   introduce a seam only where at least two adapters are actually justified
   (typically production + test/replay).
2. **The interface is the test surface.** Pure algorithm crates are tested
   through their public functions with synthetic inputs; the seams are tested by
   swapping adapters.

## 2. Layers

| Layer | Crates | Character |
| --- | --- | --- |
| L0 vocabulary | `aquila-types` | shared alphabet; no behaviour |
| L1 pure transforms | `aquila-signal`, `aquila-geometry`, `aquila-metrics` | stateless, in-process |
| L2 ingestion & models | `aquila-sources`, `aquila-inference` | I/O and model execution behind seams |
| L3 core | `aquila-sensing` | stateful pipeline: time, calibration, windows, fusion |
| L4 edges | `aquila-evaluation`, `aquila-api` | drive the core; expose the control plane |
| L5 end adapters | `aquila-ha`, `aquila-dashboard` | HA-facing output; WASM UI |

A module may only depend on modules in a strictly lower layer. The composition
root (the add-on binary, §9) depends on every layer and holds no domain logic.

## 3. Crate list

| Crate | Interface (small) | Hides (deep) |
| --- | --- | --- |
| `aquila-types` | domain newtypes and records: `NodeId`, `RoomId`, `FloorId`, `NodePair`, `Timestamp`, `Scope`, `RawCsiFrame`, `CleanCsi`, `FeatureVector`, `Correction`, `Observation`, `GroundTruthSample`, `MetricRecord` | conformance to one feature layout and one timestamp base; schema version |
| `aquila-signal` | `clean(raw, &Correction, &SignalConfig) -> CleanCsi`; `features(&[CleanCsi], &SignalConfig) -> FeatureVector` | DC/phase-offset removal, phase sanitisation, subcarrier selection, Hampel/outlier removal, bandpass, spectrogram, normalisation, feature packing |
| `aquila-geometry` | `Topology::load`, `room_of`, `floor_of`, `pairs(Scope)`, `position`, `scope_of` | the home→floor→room→node→pair graph, pair enumeration policy, spatial layout |
| `aquila-metrics` | `Metric::id/compute`; `MetricSink::record`; `diff` | metric maths (F1, PCK@20, breathing MAE), deterministic/statistical/gated taxonomy, commit-keyed ledger format, record+diff |
| `aquila-sources` | `CsiSource::next`; `NodeControl::send` | UDP framing, per-node multiplexing, node wire format both directions, replay pacing |
| `aquila-inference` | `PoseModel::infer`, `VitalsModel::infer`; presence/count heads | ONNX runtime, tensor preparation, keypoint post-processing, model version pinning |
| `aquila-sensing` | `Sensing::new/step/calibrate/sink`; `Clock`, `Sink` seams | time sync and per-node drift, whole-home calibration, sliding windows and jitter, feature-level fusion, room→floor→cross-floor escalation |
| `aquila-evaluation` | `GroundTruth::label`; `Harness::run`; `CollectorSink` | replay pacing, prediction↔ground-truth alignment in time and scope, simulator |
| `aquila-api` | `Server::bind/handle`; `Handle::publish`; `ChannelSink` | HTTP+WS framing, auth (private), serialisation, backpressure |
| `aquila-ha` | `HaSink: Sink`; `EntityModel` | HA device/entity conventions, registry shape, state payloads |
| `aquila-dashboard` | WASM entry point | rendering; a client of the `aquila-api` control plane |

## 4. Per-crate depth

### `aquila-types` (vocabulary, not a behavioural module)
This is a deliberate exception: it has an interface and almost no implementation.
It exists so the pure crates (`signal`, `geometry`, `inference`) share one
alphabet without depending on one another. In particular `Correction` lives
here so `aquila-signal` can consume calibration output without depending on
`aquila-sensing`, which produces it. No behaviour is hidden and none should be
added.

### `aquila-signal` (deep)
A stateless transform: raw CSI in, clean CSI and a packed `FeatureVector` out.
The interface is two functions; the implementation carries the CSI-specific DSP
that is the difference between "raw bytes" and "usable features". Deletion test:
remove it and every caller must re-implement phase sanitisation and the feature
layout — it earns its keep.

### `aquila-geometry` (deep)
A query surface over the topology and its spatial layout. Callers ask
`pairs(Room(3))` or `position(node)`; the graph, containment rules, and the
staged scope policy stay inside. Deletion test: remove it and `sensing`, `api`,
and `ha` each re-derive the home graph.

### `aquila-metrics` (deep)
Metric definitions plus the measurement ledger. The interface is a `Metric`
trait, a `MetricSink` seam, and a `diff`. It hides the arithmetic, the
deterministic/statistical/gated taxonomy, commit-keying, and record+diff so that
"measurement is first-class" is enforced in one place. No CI gating in v1.

### `aquila-sources` (deep, owns the core↔node seam)
One crate owns the node wire format in both directions: `CsiSource` for the
upstream CSI stream and `NodeControl` for the outbound control channel. Adapters:
`UdpCsiSource`/`UdpNodeControl` (real nodes) and `ReplayCsiSource`/
`FakeNodeControl` (fixtures). Node control is deliberately **not** a separate
seam: it is the outbound half of the same protocol and shares the same two
adapters.

### `aquila-inference` (deep, owns the model seam)
`PoseModel` and `VitalsModel` present one method each. Adapters: real ONNX models
(`OnnxPoseModel`, `OnnxVitalsModel`) and canned fakes for pipeline tests and
replay. Presence and count are simple heads over the same features in v1. The
ONNX runtime, tensor shapes, and keypoint post-processing stay inside.

### `aquila-sensing` (deepest; the stateful core)
This is the sensing core the brief makes v1. Everything the pure crates cannot
know lives here: a common time base and per-node drift, whole-home calibration,
per-node sliding windows and arrival jitter, feature-level fusion across nodes
and pairs, and the staged room→floor→cross-floor escalation. Its public
interface is deliberately tiny — construct with adapters, `step` one fused
`Observation`, `calibrate` from a sample set, `sink` the output. Internal modules
(`time`, `calibration`, `window`, `fusion`, `escalation`) have internal seams
used by the crate's own tests and are **not** exposed. Deletion test: remove it
and the stateful policy respreads across every caller.

### `aquila-evaluation` (deep, owns the ground-truth seam)
The harness that replays fixtures or a simulator through `aquila-sensing` and
aligns predictions with labels. `GroundTruth` adapters: `FixtureGroundTruth`
(MM-Fi and own captures) and `CameraGroundTruth` (research-only, but a real
second source). `CollectorSink` implements `Sink` to capture observations for
scoring.

### `aquila-api` (deep, owns the control-plane transport)
The private HTTP+WS control plane for the add-on. It is not a public interface:
no REST/WS/MCP is offered to third parties. It exposes `Server`, a `Handle`, and
a `ChannelSink` (a `Sink` that enqueues observations), and it maps observations
through `aquila-ha` before pushing. Framing, auth, serialisation, and
backpressure stay inside.

### `aquila-ha` (adapter)
The HA-facing output adapter: `HaSink` maps an `Observation` to HA entity/state
messages using `EntityModel` (home→floor→room→node→pair as HA devices/entities).
It transports nothing; `aquila-api` owns the transport. The Python custom
component consumes those messages over the control plane.

### `aquila-dashboard` (adapter)
A Rust→WASM client of `aquila-api`, served by the add-on via sidebar ingress. Its
interface is the WASM entry point; rendering is hidden. It depends only on the
`aquila-api` protocol types so the WASM boundary stays narrow.

## 5. Dependency DAG

```
                      aquila-types
                           ▲
     ┌──────────┬──────────┼──────────┬───────────┐
 aquila-      aquila-    aquila-    aquila-     aquila-
 signal       geometry   metrics    sources     inference
     ▲            ▲          ▲         ▲            ▲
     └────────────┴──────────┼─────────┴────────────┘
                             │
                      aquila-sensing  (Clock, Sink seams)
                        ▲         ▲
            ┌───────────┤         └────────────┐
     aquila-evaluation  │              aquila-ha
            │           │                 ▲
            │      aquila-api ────────────┘
            │           ▲
            │    aquila-dashboard
            │
     (aquila-metrics, aquila-sources already above)
```

Edges (each arrow points to a dependency):

- `aquila-types` → ∅
- `aquila-signal`, `aquila-geometry`, `aquila-metrics` → `aquila-types`
- `aquila-sources`, `aquila-inference` → `aquila-types`
- `aquila-sensing` → `aquila-types`, `aquila-geometry`, `aquila-signal`,
  `aquila-sources`, `aquila-inference`
- `aquila-evaluation` → `aquila-types`, `aquila-sources`, `aquila-sensing`,
  `aquila-metrics`
- `aquila-ha` → `aquila-types`, `aquila-geometry`, `aquila-sensing`
- `aquila-api` → `aquila-types`, `aquila-geometry`, `aquila-sensing`,
  `aquila-sources`, `aquila-ha`
- `aquila-dashboard` → `aquila-api`

The graph is acyclic and layer-respecting: no L1/L2 crate depends upward, and
`aquila-sensing` is the only L3 module.

### Enforcement

The DAG is enforced, not documented-and-hoped. `cargo metadata` is checked by a
small `xtask` that fails the build on any edge outside the allow-list above. Rust
crate boundaries give the wall; the `xtask` gives the direction. No plugin or
dynamic-loading seam exists — the brief keeps everything in-tree and
compile-time.

## 6. Seams: kept and deleted

### Kept (each with two real adapters)

| Seam | Owner | Adapter A (production) | Adapter B (test/replay) | Why real |
| --- | --- | --- | --- | --- |
| `CsiSource` | `aquila-sources` | `UdpCsiSource` (nodes over WiFi UDP) | `ReplayCsiSource` (recorded fixtures) | every DSP and inference test runs on fixtures; replay+sim is first |
| `Clock` | `aquila-sensing` | `SystemClock` (host monotonic + drift model) | `ReplayClock` (fixture time) | deterministic replay requires fixture-derived time |
| `PoseModel` / `VitalsModel` | `aquila-inference` | `OnnxPoseModel`, `OnnxVitalsModel` | `FakePoseModel`, `FakeVitalsModel` | real weights vs canned outputs for pipeline tests |
| `GroundTruth` | `aquila-evaluation` | `FixtureGroundTruth` | `CameraGroundTruth` | two independent label sources |
| `Sink` | `aquila-sensing` | `ChannelSink`/`HaSink` (live output) | `CollectorSink` (evaluation) | live path vs measurement capture |
| `MetricSink` | `aquila-metrics` | `LedgerMetricSink` (commit-keyed ledger) | `InMemoryMetricSink` | persisted ledger vs test assertions |

### Deleted (hypothetical seams)

- **`TopologySource`** — topology is data, loaded by `Topology::load` from one
  format. A second adapter would be invented, not needed. Inject a value.
- **`Transport`** — one private HTTP+WS control plane; abstracting HTTP vs WS is
  indirection without a second adapter.
- **`FusionStrategy`** — v1 has one feature-level fusion. A strategy seam would
  be hypothetical; when a second fusion exists, promote the internal `fusion`
  module's seam.
- **`StorageBackend`** — the ledger is the only persistence; `MetricSink` is the
  sole persistence seam.
- **`ModelRegistry`** — model selection is configuration (a weights path), not a
  port. The `PoseModel`/`VitalsModel` seam already covers variation.
- **`NodeControl` as its own seam** — folded into `aquila-sources`; it is the
  outbound half of the same wire format with the same two adapters.
- **`Clock` as its own crate** — kept as a seam, not a crate: only
  `aquila-sensing` consumes it, so it lives at that interface.
- **`EventBus` / `Notifier`** — `Sink` covers output; an extra publish seam would
  duplicate it.

## 7. The core↔node seam

Lives in `aquila-sources`. One crate owns the node wire format in both
directions:

- **Upstream (nodes → core):** `CsiSource::next` yields `RawCsiFrame`s. The
  adapter (`UdpCsiSource`) hides UDP framing, per-node demultiplexing, and the
  CSI payload encoding; `ReplayCsiSource` hides fixture pacing.
- **Downstream (core → nodes):** `NodeControl::send` delivers `NodeCommand`
  (channel, rate, calibration triggers). `UdpNodeControl` is the real adapter;
  `FakeNodeControl` records commands in tests.

Because both directions share the same two adapters and the same wire format,
this is one seam, not two. The node is a dumb sensor; all DSP and inference stay
behind this seam in the core.

## 8. The core↔HA-component seam

Lives at `aquila-api` (transport) with `aquila-ha` (mapping):

- `aquila-sensing` emits `Observation`s through the `Sink` seam.
- The production `ChannelSink` (in `aquila-api`) enqueues them.
- `aquila-api`'s `Server` maps each observation through `aquila-ha::EntityModel`
  and pushes HA entity/state messages over the private HTTP+WS control plane.
- The Python HA custom component (HACS, outside the crate graph) connects to that
  control plane; the `aquila-dashboard` WASM client is served over the same plane
  via sidebar ingress.

The second adapter for this seam exists for tests: an in-process fake control
plane lets `aquila-ha` and `aquila-api` be exercised without a network. The
control plane is private: no public REST/WS/MCP, no SDK.

## 9. Workspace layout

```
crates/
  aquila-types/  aquila-signal/  aquila-geometry/  aquila-metrics/
  aquila-sources/ aquila-inference/ aquila-sensing/ aquila-evaluation/
  aquila-api/ aquila-ha/ aquila-dashboard/
addon/            # composition root: wires adapters, serves dashboard + ingress;
                  # holds no domain logic, exposes no module interface
custom_components/aquila/   # HACS component (Python), outside the crate DAG
```

The add-on binary is the composition root: it loads `Topology`, constructs
`UdpCsiSource` + `SystemClock` + the ONNX models, builds `Sensing`, and serves
`aquila-api`. It contains no domain logic and is not itself a module.

## 10. Mapping to the settled seams

The brief names six real seams. This design keeps exactly those six and gives
each a home:

| Settled seam | Home crate |
| --- | --- |
| `CsiSource` | `aquila-sources` |
| `Clock` | trait in `aquila-types`; seam owned by `aquila-sensing` |
| `PoseModel` / `VitalsModel` | `aquila-inference` |
| `GroundTruth` | `aquila-evaluation` |
| `Sink` | `aquila-sensing` |
| `MetricSink` | `aquila-metrics` |

No seventh seam is introduced.

## 11. Revisit triggers

- A second fusion algorithm appears → promote the internal `fusion` seam to its
  own L1 crate.
- Calibration acquires an independent consumer or a second estimator → promote
  the internal `calibration` seam to its own crate.
- A second node wire protocol appears (C6, ESP-NOW) → split `aquila-sources` by
  protocol, keeping `CsiSource`/`NodeControl` in place.
- Phase-coherent RF reference becomes real (future-only) → a new L2 crate at the
  same seam as `aquila-sources`, not a change to `aquila-signal`.
