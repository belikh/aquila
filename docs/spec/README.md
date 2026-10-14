# Aquila specification — index

The specification is split one topic per file. Read `CONTEXT.md` first; it fixes the vocabulary
this spec uses. Numbers and external claims cite `../reference/sources.json`.

## Files

| Spec | Topic |
| --- | --- |
| `architecture.md` | Crate/module decomposition, the dependency DAG, and the seams. |
| `data-contracts.md` | Canonical versioned types (`CsiFrame`, `Pose`, `VitalsReading`, topology, ledger row). |
| `capture-and-firmware.md` | The `CsiSource` seam, ESP32-S3 node firmware, the UDP wire protocol, the simulator, `.aqs` sessions. |
| `firmware-ota.md` | Node firmware OTA: signed images, dual-partition A/B, rollback, and the HA `update` entity. |
| `signal.md` | The pure signal-conditioning module: decode, sanitise, filter, features. |
| `geometry-calibration.md` | Topology → zones, coordinate frames, the global whole-home calibration artefact. |
| `inference-models.md` | `PoseModel`/`VitalsModel` seam, the Python→ONNX pipeline, runtime choice (`ort`). |
| `fusion-sync.md` | `Clock` seam, software time sync, feature-level multi-node fusion. |
| `evaluation.md` | `GroundTruth` seam, accuracy protocols, the honest-claims grading scheme. |
| `measurement.md` | The `Measurement` type, `MetricSink`, metric catalogue, the commit-keyed ledger. |
| `core-api.md` | The private loopback HTTP + WebSocket control plane. |
| `dashboard.md` | The Rust→WASM management dashboard (Leptos) served via HA ingress. |
| `home-assistant.md` | The add-on packaging and the HACS component, entity model, naming. |
| `privacy-security.md` | The local-first guarantees (G1–G16) and the threat model. |
| `repository-tooling.md` | Monorepo layout, toolchain, CI, guardrail checks. |
| `interface-register.md` | The canonical crate/interface register: crate list, layers, seams, ports and the authoritative dependency edges (settled by ADR-0003). |
| `testing.md` | Test tiers, `aquila-testkit`, fixtures, the honest-claims contract. |
| `roadmap.md` | Nine tracer-bullet slices and the v1 exit criteria. |
| `risks.md` | Risk register and the open questions the tickets must resolve. |

## Resolved

The five divergences formerly recorded here — the crate graph, the node
data/control ports, `aquila-types` vs the superseded `aquila-contracts`, the
source-seam name, and the `sensing frame` definition — are **resolved** by
`../adr/0003-interface-reconciliation.md`. The canonical crate list, seams,
dependency edges, ports and contract version are registered in
`interface-register.md`; `tools/dag.toml` enforces the graph via
`cargo run -p xtask -- dag-check`. Any crate name or port not in the register is
a defect. The file table above now indexes `interface-register.md` and
`firmware-ota.md` (audit #20).
