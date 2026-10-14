# ADR-0001: Module architecture

- Date: 2026-10-13
- Status: Accepted
- Deciders: Aquila design session
- Related: `docs/spec/architecture.md`

## Context

Aquila is WiFi CSI sensing for the home: presence/occupancy, person count, coarse
pose, and breathing/heart rate. Home Assistant is host and UI. v1 is the sensing
core plus global whole-home calibration plus the HACS custom component plus a
management dashboard served by the add-on via sidebar ingress.

The brief already fixes many constraints: Linux only; ESP32-S3 reference nodes
streaming raw CSI over WiFi UDP; no ESP-NOW, no MQTT; the control plane is a
private HTTP+WS plus a direct node control channel; nodes are dumb sensors and
the add-on does all DSP, inference, fusion, calibration, and measurement; the
domain is home→floor→room→node→node-pair from day one; models seed from ruvnet
NC weights, are fine-tuned on MM-Fi and own captures, exported to ONNX, and run
in Rust; replay and simulation come first.

What the brief did not yet fix is the crate/module decomposition, the dependency
DAG, and which seams are real. The design principles we are holding to are deep
modules, small interfaces, real seams only (justified by two adapters), TDD with
fake adapters and recorded fixtures, an enforced dependency DAG, and no committed
agent scaffolding. We need a decomposition that makes the v1 measurement bars
(torso-PCK@20 ≥65%, presence F1 ≥0.9, breathing MAE ≤2 brpm) cheap to evaluate
honestly.

## Decision

Adopt the eleven-crate workspace described in `docs/spec/architecture.md`, in six
layers, with exactly the six settled seams:

```
L0 vocabulary      aquila-types
L1 pure transforms aquila-signal, aquila-geometry, aquila-metrics
L2 ingestion/models aquila-sources, aquila-inference
L3 core            aquila-sensing
L4 edges           aquila-evaluation, aquila-api
L5 end adapters    aquila-ha, aquila-dashboard
```

Key choices:

1. **`aquila-sensing` is the single stateful core.** Time sync and per-node
   drift, whole-home calibration, sliding windows and jitter, feature-level
   fusion, and room→floor→cross-floor escalation all live inside it as internal
   modules with internal seams. Its public interface is tiny: construct with
   adapters, `step`, `calibrate`, `sink`.
2. **`Clock` and `Sink` are seams at `aquila-sensing`'s interface, not crates.**
   Only the core consumes them, so they sit where they are used.
3. **`aquila-sources` owns the node wire format in both directions.** The
   upstream CSI stream (`CsiSource`) and the downstream control channel
   (`NodeControl`) share one protocol and the same two adapters, so node control
   is not a seventh seam.
4. **`aquila-api` is a private control plane,** not a public interface: no SDK,
   no REST/WS/MCP, no plugins. `aquila-ha` maps observations to HA entities and
   transports nothing; `aquila-api` owns the transport.
5. **The DAG is enforced** by a small `xtask` over `cargo metadata` against an
   allow-list of edges. Crates give the wall; the `xtask` gives the direction.
6. **Everything in-tree and compile-time.** No dynamic plugin loading, no
   service discovery, no message broker.

## Alternatives considered

1. **A single crate (or a few coarse crates).** Simplest to start, but it loses
   the enforced DAG and the seam discipline that let replay, simulation, and
   measurement run on fixtures without the network. Rejected: it makes the v1
   measurement bars harder to hold to and lets DSP, inference, and I/O tangle.

2. **Finer decomposition: separate `clock`, `calibration`, and `fusion`
   crates.** Attractive on single-responsibility grounds, but each is consumed by
   `aquila-sensing` alone; promoting them invents interfaces with one real
   consumer and risks shallow crates. Rejected for v1, with promotion triggers
   recorded in the spec (§11). Calibration and fusion remain internal modules
   with internal seams, tested directly.

3. **MQTT / event-bus / plugin architecture.** The brief rules out MQTT and
   plugins, and a publish-subscribe seam would duplicate `Sink`. Rejected: it
   adds seams with no second adapter and weakens locality.

4. **gRPC/protobuf control plane.** Heavier than needed for a private add-on
   control plane and pulls a schema toolchain into the WASM boundary. Rejected:
   private HTTP+WS is sufficient and stays narrow.

5. **Python core with Rust only for hot loops.** Rejected: one Rust workspace
   for DSP, inference, and the WASM dashboard keeps one toolchain, one set of
   domain types, and one dependency DAG.

6. **Splitting configuration/topology behind a port.** Topology is data in one
   format; a `TopologySource` seam would have a single adapter. Rejected: inject a
   value, not a port.

## Consequences

Positive:

- The six settled seams each have exactly two adapters, so playback, simulation,
  and evaluation run entirely on recorded fixtures with no nodes present.
- The DAG is mechanical to check; layer violations fail the build.
- Measurement is first-class: `aquila-metrics` owns the ledger behind
  `MetricSink`, and `aquila-evaluation` owns the only `GroundTruth` seam.
- `aquila-types` is the shared alphabet, so pure crates (`signal`, `geometry`,
  `inference`) stay mutually independent.
- `aquila-dashboard` depends only on `aquila-api`'s protocol types, keeping the
  Rust→WASM surface narrow.

Negative / accepted costs:

- `aquila-sensing` is a large crate. Its depth is deliberate; the interface stays
  small, and internal modules keep locality. If it grows past comfort, the
  promotion triggers in the spec split it without changing consumers.
- Calibration and fusion have no reusable interface outside the core, so a second
  consumer would require promotion (an accepted future cost).
- No plugin seam means C6 support and any future phase-coherent RF reference are
  in-tree additions at the `aquila-sources` seam, not drop-in extensions.
- `aquila-api` depends on both `aquila-sensing` and `aquila-ha`; this is acyclic
  but makes `aquila-api` the busiest edge crate. The composition root keeps it
  free of domain logic.

## Follow-ups

- Land the `xtask` DAG check with the first crates.
- Record the internal-module layout of `aquila-sensing` (`time`, `calibration`,
  `window`, `fusion`, `escalation`) when that crate is first implemented.
- Revisit this ADR if any promotion trigger in `docs/spec/architecture.md` fires.
