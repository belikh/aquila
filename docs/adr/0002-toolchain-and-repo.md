# ADR-0002: Toolchain and repository

- Date: 2026-10-13
- Status: Accepted
- Deciders: Aquila design session
- Related: `docs/spec/repository-tooling.md`, `docs/adr/0001-module-architecture.md`

## Context

ADR-0001 fixes the eleven-crate workspace and the dependency DAG, but it does not
decide how the repository is shaped, how the DAG is enforced, how code is
formatted, linted, licensed and released, or what history and commit discipline
the project holds. The brief leaves those to the repository, CI and tooling role
while fixing the surrounding constraints: one GPLv3 monorepo at
`belikh/aquila`; Rust for the sensing core and the Rust→WASM dashboard; Python for
the HA custom component; ESP32-S3 firmware; Linux-only hosts; a private
control plane; measurement as a first-class, commit-keyed ledger that records and
diffs but does not gate CI; no committed agent scaffolding; Australian English.

The failure we are guarding against is the one the project exists to avoid: a
pipeline with no enforced structure, where a module reaches upward, a fake drifts
from its real adapter, a licence is incompatible, an agent harness is committed,
or a "measurement" cannot be reproduced. Tooling must make those states hard to
reach and loud when reached.

## Decision

Adopt the repository layout, tooling and CI in
`docs/spec/repository-tooling.md`. In summary:

1. **One monorepo, fixed top-level shape.** `crates/` is the only Rust
   workspace (the eleven `aquila-*` modules, the `aquila-addon` composition-root
   binary, the dev-only `aquila-testkit`, and `xtask`). `firmware/`, `ha/`,
   `dashboard/`, `fixtures/`, `docs/`, `tools/` and `.github/` sit outside the
   crate graph. `Cargo.lock` is committed. `repository.yaml` and `hacs.json` stay
   at the repo root.

2. **A pinned toolchain and MSRV.** `rust-toolchain.toml` pins the compiler;
   `[workspace.package] rust-version` is kept equal to it. A toolchain bump is
   its own reviewed commit, because measurement determinism depends on it.

3. **`#![forbid(unsafe_code)]` workspace-wide**, via
   `[workspace.lints.rust] unsafe_code = "forbid"`. Any future exception needs an
   ADR, a single-crate `deny` downgrade, and localised `#[allow(unsafe_code)]`
   with `// SAFETY:` justifications. Expected unsafe count in v1 is zero.

4. **Format, lint, audit as gates.** `rustfmt` (`--check`), `clippy`
   (`-D warnings`, workspace `all` + `pedantic`), `cargo-deny` (advisories,
   licences, bans, sources; crates.io only), `cargo-machete`, and `reuse lint`
   (GPL-3.0-only SPDX headers) all run on every PR.

5. **The dependency DAG is enforced, not documented.** `cargo xtask dag-check`
   reads `tools/dag.toml` (an allow-list of edges plus per-crate levels) and
   `cargo metadata`, and fails on any unlisted edge, any upward edge, any cycle,
   any unregistered member, and any normal dependency on `aquila-testkit`. The
   check also runs as a `#[test]`.

6. **Licensing: GPL-3.0-only** (SPDX `GPL-3.0-only`) with per-file SPDX headers
   and a `REUSE.toml` for files that cannot carry a header.

7. **No committed agent scaffolding.** `.gitignore` blocks `.claude/`,
   `.claude-flow/`, `.swarm/` and friends; a CI check fails if any such path is
   tracked. In-tree fakes and fixtures are owned product code, not scaffolding.

8. **Conventional Commits and non-squashed history.** Commit messages and PR
   titles follow Conventional Commits 1.0.0; squash-merge is disabled and branch
   protection preserves individual commits. Releases are signed, immutable tags.

9. **Four named workflows** (plus two support workflows): `build.yml` (format,
   lint, DAG and scaffolding guardrails, audits, compile, WASM), `test.yml`
   (tiers 0–3, coverage floor, reference-pipeline bit-exactness, claims
   verifier), `ledger.yml` (record + diff; append on `main`), and
   `package-addon.yml` (multi-arch add-on images and the HACS `aquila.zip`
   asset), with `commits.yml` and `nightly.yml` alongside. Correctness gates;
   measurement does not.

## Alternatives considered

1. **A Cargo workspace per component (core, dashboard, add-on).** Rejected: the
   enforced DAG and the single `aquila-types` alphabet require one graph; separate
   workspaces would hide edges from the check and duplicate the vocabulary.

2. **Enforce the DAG with `cargo-deny` bans or `cargo-depgraph` only.** Rejected:
   bans express "forbid crate X", not "forbid direction"; a hand-written `xtask`
   over `cargo metadata` states the layer rule directly and yields an actionable
   message naming the ADR that owns a new edge.

3. **Nightly rustfmt / unstable options.** Rejected: it couples formatting to a
   moving compiler and undercuts the pinned-toolchain determinism argument.

4. **`cargo test` alone, without `nextest`.** Rejected for CI: `nextest` gives
   per-test process isolation, retries that expose flakiness, and machine-readable
   results the measurement ledger consumes (`testing.md` §4.1).

5. **Squash-merge for a tidy `main`.** Rejected: it destroys the per-commit
   history that commit-keyed measurement, `git diff` ledger work, and honest
   bisection rely on. Tidy is not worth the loss of provenance.

6. **Commit generated agent harnesses for convenience.** Rejected by the brief
   and by locality: generated, unreviewed trees drift from the contracts they
   claim to test.

7. **A public release pipeline (crates.io / public images).** Rejected for v1:
   the workspace is `publish = false`, the API is private, and only the add-on
   and HACS asset are published.

## Consequences

Positive:

- The dependency direction is mechanical to check and hard to violate; the
  failure message names the owning ADR.
- Formatting, lints, licences and advisories cannot drift silently.
- Determinism is anchored: a pinned toolchain, a committed lockfile, canonical
  ledger serialisation, and a bit-exact reference-pipeline asset.
- History is honest and legible: conventional commits, real commits, signed tags.
- The repository is reproducible from a tag without hidden local state.

Negative / accepted costs:

- `tools/dag.toml` is a second place the crate graph is written down; it can go
  stale, so an unregistered member fails the build and a rename is a reviewed
  one-file change against ADR-0001. This is the price of a data-driven check.
- Pinning third-party actions to commit SHAs adds Dependabot maintenance.
- `cargo-deny`'s `multiple-versions = "warn"` and the curated licence allow-list
  will need periodic tightening.
- `#![forbid(unsafe_code)]` will force an ADR if a future coherent-array or FFI
  shim needs `unsafe`; that friction is intended.
- Multi-arch container builds (amd64 + aarch64 with QEMU) are slow and heavier
  than a single-arch build; the reference deployment includes arm64 hosts, so it
  is required.

## Follow-ups

- Land `xtask` (`dag-check`, `check-scaffolding`, `coverage-floor`,
  `claims-verify`) with the first crates, per ADR-0001. (`ledger-comment` was
  dropped: the ledger diff is posted with `gh pr comment`, not a bespoke
  subcommand — `sister-issues-specs.md` D41.)
- Reconcile the three flagged divergences in `docs/spec/repository-tooling.md`
  §13 with ADR-0001 and the measurement owner: the `architecture.md` §2/§5 layer
  labels, the ledger path, and the HA tree location.
- Correct `docs/spec/architecture.md` §2 (or bless the derived levels) so the
  documented layers match the enforced DAG.
- Revisit this ADR if the crate graph is regrouped or a toolchain-floor change is
  needed.
