# 06 — Floor aggregation: room → floor

- Type: task
- Blocked by: #5
- Status: open

## Goal

Room-level outputs compose into per-floor presence, occupancy and count, with
room attribution recorded. The hierarchy is data, not hard-coded topology.

## Implements

- `docs/spec/roadmap.md` Slice 5.
- `docs/spec/fusion-sync.md` §2.5 (soft-OR for occupancy, convolution for count,
  staging rules, `Unknown` coverage).
- `docs/spec/geometry-calibration.md` §4 (staging table, `floor_of`), §5 (stated
  bounds).
- `docs/spec/data-contracts.md` §3 (`Floor`, `Room`, `Topology`).
- `docs/spec/evaluation.md` §5.3 (cross-room is reported-but-ungated).

## References

`mmfi` (multi-environment precedent). No external accuracy claim; in-domain only.

## Acceptance criteria

- **Floor presence gate:** `presence.f1.floor` ≥ 0.90 in-domain.
- **Attribution recorded:** `attribution.accuracy.room` is recorded and
  baselined; no cross-room leakage regression on presence.
- **Honesty gate:** cross-room results are reported-but-ungated and explicitly
  labelled; ambiguity returns `Conflict::AmbiguousRooms`, never a false confident
  room (risks.md R-01, Q-01).
- **Data-driven gate:** topology comes from `Topology::load`, never hard-coded.

## Test-first first step

Write a failing test that fuses two rooms on one floor and asserts the floor
occupancy soft-OR and the convolved count PMF, plus a constructed ambiguous case
that raises the expected conflict.
