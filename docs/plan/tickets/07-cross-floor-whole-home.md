# 06 — Cross-floor / whole-home

- Type: task
- Blocked by: 05
- Status: open

## Goal

Whole-home presence, count and coarse location across floors, with cross-floor
fusion and calibration intact at the reference ~9+ node scale, reported honestly
as in-domain.

## Implements

- `docs/spec/roadmap.md` Slice 6.
- `docs/spec/fusion-sync.md` §2.5 (home soft-OR and convolution), §1.5
  (accuracy budget across nodes).
- `docs/spec/geometry-calibration.md` §4 staging, §10 (whole-home
  re-application), §11 quality.
- `docs/spec/capture-and-firmware.md` §7 (three-floor simulator scene).
- `docs/spec/evaluation.md` §6 (`presence.f1.home`, `count.mae.home`).

## References

`espargos` (why coherence is out of scope), `mmfi`, `csibench`. In-domain only;
no cross-environment claim.

## Acceptance criteria

- **Home presence gate:** `presence.f1.home` ≥ 0.90 at reference scale.
- **Count recorded:** `count.mae.home` recorded within the band fixed by the
  ticket-04 baseline (risks.md R-04); not gated.
- **Calibration gate:** calibration re-applies across the full topology with no
  per-floor special cases.
- **Staging gate:** cross-floor fusion stays off until per-floor calibration
  passes; uncalibrated coverage reports `Unknown`, never `Absent`/`0`.
- **Scale gate:** clock-grid alignment is proven at reference scale in sim before
  any live claim (risks.md R-24).

## Test-first first step

Write a failing tier-3 simulator test with three floors and 9+ nodes that
asserts whole-home presence and a convolved count PMF equal to the floor-wise
convolution.
