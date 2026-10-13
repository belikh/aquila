# 02 — Vitals tracer bullet: breathing and heart rate on the same pipeline

- Type: task
- Blocked by: 01
- Status: open

## Goal

Breathing and heart rate ride the same frames and clock as presence, from
fixture to output, scored against a reference trace. The `VitalsModel` seam
appears with a deterministic fake first, then the ONNX adapter behind it.

## Implements

- `docs/spec/roadmap.md` Slice 2.
- `docs/spec/inference-models.md` §1 (`VitalsModel` seam, fakes), §4 (loading,
  warm-up, failure — never fabricate), §5–6 (measurement, licence).
- `docs/spec/signal.md` §4.7–4.10 (bandpass, STFT, `bvp_track`).
- `docs/spec/data-contracts.md` §5 (`VitalsReading`, `Estimate`, `VitalsStatus`).
- `docs/spec/evaluation.md` §4.3 (breathing MAE, Bland-Altman, heart baseline).
- `docs/spec/measurement.md` §4.6 (`accuracy.breathing.mae_brpm`).

## References

`csibench`/`csibench-paper` (vitals benchmark), `ort` (runtime), `mmfi`
(provenance). Heart-rate reference is a worn chest strap (evaluation.md §2.2).

## Acceptance criteria

- **Breathing gate:** `breathing.mae` ≤ 2 brpm against the fixture reference.
- **Heart baseline:** heart-rate MAE is recorded, then its v1 bar is **fixed from
  this baseline** and frozen in the ledger (risks.md R-05); no invented target.
- **Same-path gate:** vitals are computed on the same frames and clock as
  presence, not a parallel path (assert shared `SensingFrame` input).
- **Honesty:** `status != Ok` never co-reports a value; `Claimed` readings are
  recorded but never surfaced; a failed model run yields no reading.
- Ledger rows `breathing.mae`, `heart.mae`, `vitals.window.count`.

## Test-first first step

Write a failing tier-1 test that replays a fixture with a known breathing
reference and asserts a `VitalsReading` whose breathing estimate is within the
bar, driving `FakeVitalsModel` first.
