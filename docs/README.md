# Aquila — documentation index

Everything an implementation agent needs is under `docs/`. Read `CONTEXT.md` first, then the
spec index, then the ADRs relevant to what you are touching.

To install and operate Aquila, see [`INSTALL.md`](INSTALL.md). For the honest
state of v1 (what is verifiably done and what remains hardware-gated), see the
"v1 status" section of the top-level [`README.md`](../README.md).

## Layout

| Path | What lives there | Authority |
| --- | --- | --- |
| `CONTEXT.md` | Domain glossary. The canonical vocabulary. | Normative — use these terms exactly |
| `docs/spec/` | The specification. Index at `docs/spec/README.md`. | Normative |
| `docs/adr/` | Architecture decision records. | Normative; immutable once accepted |
| `docs/research/` | Investigations (datasets, hardware, options). | Advisory and out-of-band; never defines v1 scope |
| `docs/reference/` | External resources: datasets, papers, tools, licences. | Reference; `sources.json` is machine-readable |
| `docs/plan/` | The wayfinder map and the tickets. | Normative for delivery |
| `docs/agents/` | Skill configuration: tracker, triage labels, domain-consumption rules. | Normative for agent workflows |
| `docs/research/esp32-as-sdr.md` and friends | Individual research notes, one question each. | Advisory |

## Reading order for a new task

1. `CONTEXT.md` — learn the language.
2. `docs/spec/README.md` — find the spec files for your area and the known divergences.
3. The relevant `docs/spec/*.md` and `docs/adr/*.md`.
4. `docs/reference/sources.json` — cite any external claim you repeat.

## Conventions

- **One question or topic per file.** Filenames are lowercase, hyphenated, and stable.
- **Australian English** in all prose (behaviour, colour, centre, optimise, prioritise).
- **Honest claims only.** Every factual claim cites `docs/reference/` or a spec/test; numbers
  carry one of the five evidence grades, strongest first: gated / measured / claimed /
  data-gated / hardware-gated.
- **Research is out-of-band.** No file under `docs/research/` can add, remove or reshape v1 scope;
  it feeds the future maps, not the current one. Surface any conflict rather than silently following it.
- **No agent scaffolding in the repo.** `.claude`, `.claude-flow`, `.swarm` are ignored and CI-checked.
- **Licence:** GPL-3.0-only code; the project is non-commercial, so non-commercial (CC BY-NC)
  datasets and weights are usable but must never be redistributed as if permissive.

## Claims

The machine-readable registry (`../claims.toml`) and this table are a
**bijection**: every registry claim appears here, and every row here has a
registry entry. `cargo run -p xtask -- claims-verify` parses both and fails if
they diverge (`testing.md` §7). None of the v1 bars is measured yet, so every
row is `target`.

| Claim id | What it asserts | Metric | Status |
| --- | --- | --- | --- |
| `CLM-torso-pck20` | In-domain torso PCK@20 is at or above 65% on single-room pose fixtures. | `accuracy.torso_pck_at_20` | target |
| `CLM-presence-f1` | In-domain presence F1 is at or above 0.90 per zone (macro). | `accuracy.presence_f1` | target |
| `CLM-breathing-mae` | In-domain breathing MAE is at or below 2 brpm against the respiratory-belt reference. | `accuracy.breathing.mae_brpm` | target |

## Status

- Specs: 19 topic files (plus `docs/spec/README.md`), including `firmware-ota.md`; the
  crate graph and ports are reconciled by ADR-0003 and `docs/spec/interface-register.md`.
- ADRs: `0001`–`0006`.
- Research: paired datasets, plugin architectures, ESP32-as-SDR, ESP32-C5 dual-band, node backhaul.
- Plan: v1 wayfinder map (#1) and tickets published at https://github.com/belikh/aquila/issues;
  `docs/plan/` mirrors them (`docs/plan/issues.md` maps plan numbers to issue numbers).
- Firmware OTA is **implemented and unit-tested but not hardware-verified**: #13 (core OTA control
  channel + signed image store), #14 (ESP32-S3 A/B OTA agent with Ed25519 and rollback) and #15
  (Home Assistant `update` entity) have landed. The Rust/control paths and the Python entity are
  tested; the firmware compiles but has never been flashed or run an OTA on a device.
- Packaging and release automation for a reproducible add-on image and a HACS `aquila.zip` asset are
  prepared (`.github/workflows/`, `aquila_core/Dockerfile`, `tools/package-hacs.sh`). The image is
  inspection-validated only in this environment (no container engine); no `v1.0.0` tag or release
  has been created, because the v1 accuracy bars remain hardware-gated. See the top-level
  [`README.md`](../README.md) "v1 status".
