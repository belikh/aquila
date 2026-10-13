# 04 — Global whole-home calibration

- Type: task
- Blocked by: #3
- Status: open

## Goal

A calibration pass over the whole home emits a first-class, commit-keyed
calibration artefact that later sensing consumes, removing per-node and
per-session offsets. The calibration reaches the signal module only as data
(`Correction`), never as a crate dependency.

## Implements

- `docs/spec/roadmap.md` Slice 3.
- `docs/spec/geometry-calibration.md` §7 (what global calibration learns), §9
  (artefact and storage), §10 (versioning and invalidation), §11 (quality
  metrics).
- `docs/spec/capture-and-firmware.md` §4 (`Clock` seam, per-node drift).
- `docs/spec/fusion-sync.md` §1.2–1.6 (offset estimation, drift fit, accuracy
  budget).
- `docs/spec/data-contracts.md` §10 (`Calibration`, `Baseline`, `ClockModel`).
- `docs/spec/signal.md` §4.4 (normalisation consumes `Correction`).

## References

`esp-csi` (reference firmware for capture metadata). Time-sync budget per
`fusion-sync.md` §1.5.

## Acceptance criteria

- **Versioned artefact gate:** calibration is a versioned artefact referenced by
  commit and `topology_digest`, never an implicit global.
- **Reproducibility gate:** re-running calibration on the same fixtures
  reproduces the same artefact digest (`calibration.digest`).
- **Cross-session gate:** presence F1 variance across sessions drops versus the
  uncalibrated baseline; vital MAE does not regress (risks.md R-02).
- **Global-by-default gate:** whole-home calibration precedes any room-local
  tuning; an invalid/stale artefact is never silently used.
- Ledger rows `calibration.digest`, `presence.f1.cross_session`,
  `calibration.drift.per_node`.

## Test-first first step

Write a failing test that injects a known per-node skew into two fixture
sessions and asserts the calibration artefact recovers and inverts it, with a
stable digest.
