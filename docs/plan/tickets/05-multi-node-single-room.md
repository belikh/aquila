# 04 — Multi-node single room: fusion and coarse pose

- Type: task
- Blocked by: 03
- Status: open

## Goal

Two or more nodes in one room fuse at the measurement level so spatial diversity
steadies presence and yields a coarse pose, with inter-node drift estimated and
applied. `PoseModel` appears with a fake then the ONNX adapter.

## Implements

- `docs/spec/roadmap.md` Slice 4.
- `docs/spec/fusion-sync.md` §2 (log-odds presence pooling, count mixture,
  quality weighting, conflicts, determinism invariants).
- `docs/spec/geometry-calibration.md` §4–5 (attribution, proximity fingerprint,
  bounded trilateration, stated bounds).
- `docs/spec/inference-models.md` §1.3–1.4 (`PoseModel`, shapes, post-processing).
- `docs/spec/data-contracts.md` §4 (`Pose`, COCO-17), §3 (`NodePair`).
- `docs/spec/evaluation.md` §4.1 (torso-PCK@20), §6 (v1 bars).

## References

`mmfi`, `hf-mmfi-pose` (pose seed), `ort` (runtime); `piw3d` (cross-check only).

## Acceptance criteria

- **Pose gate:** coarse torso PCK@20 ≥ 65 % in-domain for the single-room fixture
  set (`pose.pck20.torso`).
- **Presence holds:** `presence.f1.room` holds or improves against the
  single-node baseline.
- **Drift gate:** inter-node drift is estimated and applied, not assumed away.
- **Fusion gate:** fusion is feature-level; no cross-node raw-phase combination
  (fusion-sync.md §3); `Coherence::Uncoherent` only.
- **Determinism:** `fuse` is permutation-invariant and zero-weight inert
  (fusion-sync.md §2.7).

## Test-first first step

Write a failing test that fuses two `NodeEvidence` records for one room and
asserts the pooled presence probability and a pose outcome at or above the
in-domain bar, with permutation invariance.
