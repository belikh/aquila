# 01 — Spine tracer bullet: recorded CsiSource → signal → presence + count

- Type: task
- Blocked by: 11
- Status: open

## Goal

Cut the whole pipeline end to end on recorded data, measured from the first
commit: a recorded Session replays through `CsiSource` → `aquila-signal` →
presence and count decisions through `aquila-sensing`, writes an `Event` to an
in-memory `Sink`, and records a commit-keyed `MetricSink` row. This is the spine
every later slice widens; it runs on fixtures, with no hardware.

## Implements

- `docs/spec/roadmap.md` Slice 1 (exit criteria and ledger rows).
- `docs/spec/capture-and-firmware.md` §3 (`CsiSource` seam, `ReplaySource`,
  `Deterministic` mode), §8 (Session container).
- `docs/spec/signal.md` (`process`, decode/normalise/feature stages).
- `docs/spec/data-contracts.md` §1–2 (`CsiFrame`, `CsiEnvelope`, `SensingFrame`),
  §6 (`Detection`/`Event`), §7 (`Session`, `LedgerRecord`).
- `docs/spec/measurement.md` §2–3 (`Measurement`, `MetricSink`).
- `docs/spec/testing.md` §2–5 (tiers 0–2, `aquila-testkit`, reference pipeline).
- `docs/spec/interface-register.md`; ADRs `0001`, `0002`, `0003`.

## References

`proptest` (property tests), `criterion` (benches), `cargo-mutants` (nightly),
`mmfi` (fixture provenance precedent).

## Acceptance criteria

- **Deterministic replay gate:** an identical recorded Session replayed in
  `Deterministic` mode produces byte-identical frames and an identical
  `replay.digest`.
- **Presence gate:** `presence.f1` ≥ 0.90 on the in-domain fixture set (macro-F1
  across zones, `docs/spec/evaluation.md` §4.2).
- **Count recorded, not gated:** `presence.count.mae` is written as a
  `measured`/statistical row; it is never promoted to `Gated` (risks.md R-04).
- **Ledger:** one diffable `LedgerRecord` per commit for `presence.f1`,
  `presence.count.mae`, `replay.digest`; the reference-pipeline SHA-256 is pinned.
- **Honesty:** no `synthetic` fixture backs a presence claim; every gated number
  states its split, seed and coverage.
- TDD throughout; fakes and fixtures only; no hardware.

## Test-first first step

Write a failing tier-0 test that feeds a committed Session through a
`ReplayCsiSource` and asserts a presence `Detection` for a known-occupied window,
naming the interface before implementation exists.
