# Aquila — documentation index

Everything an implementation agent needs is under `docs/`. Read `CONTEXT.md` first, then the
spec index, then the ADRs relevant to what you are touching.

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
  carry an evidence grade (measured / claimed / data-gated / hardware-gated).
- **Research is out-of-band.** No file under `docs/research/` can add, remove or reshape v1 scope;
  it feeds the future maps, not the current one. Surface any conflict rather than silently following it.
- **No agent scaffolding in the repo.** `.claude`, `.claude-flow`, `.swarm` are ignored and CI-checked.
- **Licence:** GPL-3.0-only code; the project is non-commercial, so non-commercial (CC BY-NC)
  datasets and weights are usable but must never be redistributed as if permissive.

## Status

- Specs: 18 files, including `firmware-ota.md`; the crate graph and ports are reconciled by
  ADR-0003 and `docs/spec/interface-register.md`.
- ADRs: `0001`–`0006`.
- Research: paired datasets, plugin architectures, ESP32-as-SDR, ESP32-C5 dual-band, node backhaul.
- Plan: v1 wayfinder map (#1) and tickets published at https://github.com/belikh/aquila/issues;
  `docs/plan/` mirrors them (`docs/plan/issues.md` maps plan numbers to issue numbers).
- Firmware OTA is specified and ticketed: #13 (control channel + core image store), #14 (ESP32-S3
  OTA agent), #15 (Home Assistant `update` entity).
