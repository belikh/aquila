# 10 — Packaging and v1 release

- Type: task
- Blocked by: 08, 09
- Status: open

## Goal

A reproducible add-on image, an installable HACS component, a documented
install, and the honest-claims statement — everything needed to call v1 shipped
on the reference deployment (Linux, HA OS, ~9+ nodes over three floors).

## Implements

- `docs/spec/roadmap.md` Slice 9 and §"What v1 shipped means".
- `docs/spec/home-assistant.md` §3.1 (add-on config, arches, persistence), §9
  (install/upgrade, backup/restore).
- `docs/spec/repository-tooling.md` §10.4 (`package-addon.yml`), §12
  (determinism and reproducibility).
- `docs/spec/evaluation.md` §5–6 (grading scheme, v1 bars), §8 (reproducer).
- `docs/spec/testing.md` §7 (honest-claims registry bijection).
- `docs/spec/measurement.md` §5 (record + diff; nothing gates CI).

## References

`ha-addons` (add-on packaging), `hacs` (asset release).

## Acceptance criteria

- **Clean-install gate:** a clean HA OS install adds the repository, installs the
  add-on and component, and reaches a working sensing pipeline on real nodes.
- **Reproducibility gate:** the build is reproducible from a tagged commit
  (`release.build.digest`); base images and toolchains are pinned; `${HOME}` and
  secrets are never baked into committed files.
- **Bars gate:** the three in-domain v1 bars pass on the reference deployment,
  evidenced by commit-keyed ledger rows; `v1.bars.pass` is a `Gated` record. No
  headline claim outruns the ledger (risks.md R-30, Q-24).
- **Claims gate:** every README/release headline claim is `Gated` and cites a
  ledger row on a named split and seed; the claims registry and tests are a
  bijection.
- **Scaffolding gate:** no committed agent scaffolding; `check-scaffolding`
  passes.
- **Honesty gate:** v1 makes no out-of-domain, cross-environment or cross-room
  accuracy claim.

## Test-first first step

Write the claims-verifier test that fails while any README headline claim lacks a
registry entry or a backing ledger row, then make it pass.
