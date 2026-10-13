# 12 — Interface reconciliation

- Type: task
- Blocked by: none
- Status: open

## Goal

Land the canonical interface in the repository so every later ticket builds on
one crate graph and one port pair: scaffold the fourteen product crates and four
tooling crates, pin the toolchain, and encode the dependency DAG so it is
enforced, not documented.

## Implements

- `docs/adr/0003-interface-reconciliation.md` (the decision).
- `docs/spec/interface-register.md` (crate list, seams, ports, edges, version).
- `docs/adr/0002-toolchain-and-repo.md` and `docs/spec/repository-tooling.md`
  §1–5 (layout, toolchain pin, `tools/dag.toml`, `xtask dag-check`).
- `docs/spec/architecture.md` §5 (the DAG rationale).
- `docs/spec/README.md` → Resolved (divergences closed).

## References

None external. Internal authority: ADR-0003, the register.

## Acceptance criteria

- **Graph gate:** all fourteen product crates and four tooling crates exist as
  workspace members with the register's edges; `tools/dag.toml` matches the
  register exactly.
- **Enforcement gate:** `cargo xtask dag-check` fails on any unlisted edge, any
  upward edge, any cycle, any unregistered member, and any normal dependency on
  `aquila-testkit`; it also runs as a `#[test]`.
- **Toolchain gate:** `rust-toolchain.toml` pins the compiler and
  `[workspace.package].rust-version` equals it; `Cargo.lock` is committed;
  `#![forbid(unsafe_code)]` is workspace-wide.
- **Name gate:** no competing crate name (`aquila-contracts`, `aquila-source`,
  `aquila-core`, `aquila-timesync`, `aquila-fusion`, `aquila-calibration`,
  `aquila-units`, `aquila-model`, `aquila-home`, `aquila-host`, `aquila-measure`,
  `aquila-topology`, `aquila-csi`) survives in code; superseded names in sibling
  specs are flagged for correction.
- **Port gate:** the register's ports (`9100`/`9101` UDP, `8099` TCP) are the
  only ones referenced by code and config.
- **Scaffolding gate:** `cargo xtask check-scaffolding` passes.

## Test-first first step

Write the `xtask` DAG test first: assert the current (empty) workspace fails the
register's expected edges, then scaffold the crates until the test passes.
