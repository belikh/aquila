# Remediation plan — sister issues

Consolidated from the three read-only audits:
[`sister-issues-core.md`](sister-issues-core.md) (S1–S20),
[`sister-issues-firmware-ha.md`](sister-issues-firmware-ha.md) (F1–F39),
[`sister-issues-specs.md`](sister-issues-specs.md) (D1–D60).
Read each report for evidence. This file is the ordered worklist.

Rule (from `docs/agents/orchestration.md`): **one sequential writer per wave**,
commit each wave, keep `nix develop --command cargo test --workspace` green and
`cargo run -p xtask -- dag-check` passing.

## R1 — Representation & wire (the second #36) — CRITICAL first

The CSI sample's **I/Q byte order** is frozen two contradictory ways, so live
phase is conjugated; plus a dropped impairment flag and a boot-width mismatch.
- **S1 / F17** — pin one I/Q order (hardware-native, as esp-csi emits) in
  `data-contracts.md` §2, an ADR, `aquila-types`, `aquila-wire`, both
  decoders in `aquila-signal`, the simulator and the firmware; add a
  **phase-sign end-to-end test** from a hardware-shaped capture.
- **S2 / S3** — `first_word_invalid` is 4 bytes per the firmware spec but 2
  everywhere in code, and the flag is dropped from `CsiFrame`/wire; restore it
  (one complex sample) so the core can see impaired captures.
- **F18** — `boot` is `u32` on the control plane, `u16` on the wire; unify.
- **D24 / D25** — delete the superseded seam `CsiFrame` in
  `capture-and-firmware.md` §3.1; point to `aquila-types`.

## R2 — Core algorithm honesty

- **S4 (second #36)** — the pose estimator is keyed to the simulator's synthetic
  3/5/7/9 Hz dither, so the gated PCK@20 test is circular. Randomise the
  simulator's scatterer motion per seed and generalise the estimator, and grade
  the bar as **synthetic-fixture**, never hardware.
- **S5** — the v1 gated bars are asserted on synthetic-only fixtures; grade the
  ledger rows as synthetic (`Witness`) not hardware.
- **S6** — subcarrier-selection noise power is hardcoded `vec![0.0; n]`, so "SNR
  selection" is fake; implement real noise power or remove the claim.
- **S8** — pose DSP and torso fusion live in the stateful core, bypassing the
  `PoseModel` seam; route through the seam.
- **S9** — the presence threshold is tuned to the simulator's amplitude scale yet
  is the production default; make it calibration-derived.
- **S10** — the simulator's int8 quantisation saturates and fabricates RSSI.
- **S14** — all-zero `config_hash`/firmware/ground-truth SHA presented as content
  addresses; use `Option`/absent.
- Remaining: S7, S11, S12, S13, S15–S20.

## R3 — API & HA contract

- **D13 / F3** — three incompatible endpoint sets across `core-api.md`,
  `dashboard.md`, `home-assistant.md`, and the HA component calls endpoints the
  core does not serve (WS 501, REST 404). Pick one set and implement it in
  `aquila-api`.
- **D14** — `core-api.md` requires auth on every request; `home-assistant.md`
  defaults to "no token". Reconcile.
- **D41 / D42** — `xtask` subcommands (coverage-floor, claims-verify,
  ledger-comment, eval) required by specs but unimplemented; `aquila-dashboard`
  is in the register + `dag.toml` but has no crate.

## R4 — Firmware / HA security & correctness

- **F4** — `node_control.c` reads a non-existent field: the firmware cannot
  compile (proving it was never built); fix.
- **F21 / F22** — node control and CSI ingest are unauthenticated/unsealed,
  contradicting G3/G7; implement or explicitly downgrade the guarantees.
- **F1 / F23** — WiFi credentials baked in via Kconfig vs NVS provisioning + G13.
- **F24** — add-on `retention_days: 30` imposes retention, contradicting G9.
- **F27** — add-on Dockerfile is an unrunnable placeholder (no `ENTRYPOINT`).
- **F3, F6, F18, others** per the report.
- **Not defects:** F29/F39 (OTA unimplemented) — OTA is ticketed (#13–#15); it is
  the plan, not a gap.

## R5 — Spec sweep

One pass to propagate the ADR-0003/0004/0005 reconciliations that never reached
second-order docs, and to fix the rest:
- **D2** — `Witness` three vs five across `CONTEXT.md` and the specs.
- **D1 / D36** — two incompatible `.aqs` containers; delete the postcard variant.
- **D8 / D10** — `Calibration` has three shapes; ledger row and metric names
  disagree across four specs.
- **D17 / D29** — `architecture.md` and `repository-tooling.md` still show the
  pre-ADR-0003 crate graph and obsolete `tools/dag.toml` levels.
- **D26** — `torso-PCK@20` uses "neck"/"pelvis", absent from COCO-17.
- **D30** — pinned Rust 1.99.0 vs 1.98.1. **D31/D32** — spec/ADR counts.
- **D37** — uncited pose band. **D59** — ADRs still `Status: Proposed`.
- Remaining D3–D7, D9, D11, D12, D15, D16, D18–D23, D27, D28, D33–D35, D38–D58.

## R6 — Verify and resume

- Re-run the three audits against the fixed tree (expect near-zero).
- Then resume the remaining frontier: **#10 (dashboard)**, **#13/#14/#15 (OTA)**,
  **#11 (packaging)**.
