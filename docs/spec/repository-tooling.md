# Aquila repository, CI and tooling

Status: settled v1. Owner: repository, CI & tooling. Prose: Australian English.
Related: `docs/adr/0001-module-architecture.md` (the crate graph and the DAG edge
allow-list), `docs/adr/0002-toolchain-and-repo.md` (the decisions recorded here),
`docs/spec/architecture.md`, `docs/spec/testing.md` (CI topology and test tiers),
`docs/spec/measurement.md` (the ledger and `aquila-metrics`),
`docs/spec/home-assistant.md` (add-on and HACS packaging).

This document is the single source of truth for **how the repository is shaped,
built, checked and released**. It does not restate the architecture, the DSP, the
measurement catalogue or the HA entity model; those live in their own specs. Where
this document and a sibling spec disagree, the disagreement is listed in §13 and
resolved by ADR.

Non-goals: no plugin or dynamic-loading build, no second language runtime beyond
Rust (core + WASM), Python (HA component) and C (firmware); no public API
artefacts; no committed agent scaffolding; no CI gate on measurement (record and
diff only).

---

## 1. Repository shape

One monorepo at `belikh/aquila`. Top-level directories are fixed; a new top-level
directory is an ADR-level change.

```
aquila/
├── Cargo.toml               # virtual workspace manifest; workspace lints; members
├── Cargo.lock               # committed: the workspace produces binaries/images
├── rust-toolchain.toml      # the single pinned toolchain (source of truth)
├── rustfmt.toml
├── clippy.toml
├── deny.toml                # cargo-deny: advisories, licences, bans, sources
├── .editorconfig
├── .gitattributes
├── .gitignore
├── REUSE.toml               # blanket SPDX metadata for non-source files
├── LICENSES/                # GPL-3.0-only text, verbatim
├── LICENSE                  # project licence (GPL-3.0-only)
├── README.md
├── CONTEXT.md               # canonical vocabulary
├── AGENTS.md
├── claims.toml              # honest-claims registry (testing.md §7)
├── repository.yaml          # marks the repo as a Home Assistant add-on repository
├── hacs.json                # marks the repo as a HACS repository
├── crates/                  # the aquila-* Rust modules + composition root
├── firmware/                # ESP32-S3 firmware (ESP-IDF, C) — outside the crate DAG
├── ha/
│   ├── aquila_core/         # the add-on: config.yaml, Dockerfile, build.yaml, run.sh
│   └── custom_components/aquila/   # HACS component (Python) — outside the crate DAG
├── dashboard/               # WASM bundle glue + committed build metadata
├── fixtures/                # recorded/synthetic fixtures + reference pipeline asset
├── .aquila/                 # runtime state: the commit-keyed ledger (see §1.3)
├── tools/
│   ├── dag.toml             # dependency-DAG allow-list (the guardrail's data)
│   ├── coverage-floor.txt   # whole-workspace line-coverage floor (coverage-floor)
│   ├── no-agent-scaffolding.txt
│   └── versions.env         # pinned external tool versions used by CI
├── docs/
│   ├── spec/
│   ├── adr/
│   ├── research/
│   └── agents/              # human-authored process docs (issue tracker, etc.)
├── xtask/                   # repo tooling crate (dag-check, check-scaffolding,
│                            #   coverage-floor, claims-verify)
└── .github/
    └── workflows/
```

### 1.1 Why these boundaries

- **`crates/`** is the only Rust workspace. Everything Rust builds from here, so
  `cargo metadata`, the DAG check and `cargo-deny` see one graph.
- **`firmware/`**, **`ha/`** and **`dashboard/`** are *not* workspace members.
  The firmware is a separate ESP-IDF build; the HA component is Python; the
  dashboard directory holds only the WASM bundle and its bundling glue (the
  `aquila-dashboard` *crate* lives in `crates/`). Keeping them out of `crates/`
  keeps the crate graph honest.
- **`.aquila/`** holds runtime state. Only `.aquila/metrics/ledger.jsonl` (and
  its derived `index.json`) is ever committed; everything else under it is
  ignored. See §1.3.

### 1.2 Layout reconciliation

Three settled sources describe the HA/add-on tree differently, and this document
resolves them:

| Source | Draws | Resolved here as |
| --- | --- | --- |
| Brief (top authority) | `firmware/`, `ha/ python component`, `dashboard/` | top-level `firmware/`, `ha/`, `dashboard/` |
| `architecture.md` §9 | composition root at `addon/`, component at `custom_components/aquila/` | composition root binary crate `crates/aquila-addon/`; component at `ha/custom_components/aquila/` |
| `home-assistant.md` §3.3 | `repository.yaml`, `hacs.json`, `aquila_core/`, `custom_components/aquila/` | `repository.yaml` and `hacs.json` at the repo root (required by the stores); packaging at `ha/aquila_core/`; component at `ha/custom_components/aquila/` |

Rationale: `repository.yaml` and `hacs.json` **must** sit at the repository root
(the add-on store and HACS look only there), and HACS uses `zip_release` with
`filename: aquila.zip`, so the component's *repository* path is independent of its
*HA runtime* path (`custom_components/aquila`). The brief's `ha/` therefore holds
both HA-facing trees, and the one Rust composition-root binary lives in the one
Rust workspace as `crates/aquila-addon`.

### 1.3 The committed ledger

The commit-keyed measurement ledger is `.aquila/metrics/ledger.jsonl`
(JSON Lines, append-only, canonical key order), per `docs/spec/measurement.md`.
`index.json` is a derived cache and may be regenerated. `baseline.json` pins a
diff baseline. The ledger is the only part of `.aquila/` that is committed.

> Reconciliation: `data-contracts.md` §9.2 names the ledger `measure/ledger.jsonl`.
> The measurement crate (`aquila-metrics`) owns the ledger, so this document
> follows `measurement.md` and lists the divergence in §13.

---

## 2. Rust workspace

`Cargo.toml` is a **virtual manifest** (no root package). It pins the members,
the shared lints, and the resolver.

```toml
[workspace]
resolver = "2"
members = [
    "crates/aquila-types", "crates/aquila-signal", "crates/aquila-geometry",
    "crates/aquila-metrics", "crates/aquila-sources", "crates/aquila-inference",
    "crates/aquila-sensing", "crates/aquila-evaluation", "crates/aquila-api",
    "crates/aquila-ha", "crates/aquila-dashboard",
    "crates/aquila-addon",   # composition root (binary)
    "crates/aquila-testkit", # dev-only fixtures/fakes (testing.md §3)
    "xtask",
]
exclude = ["firmware", "ha", "dashboard", "fixtures"]

[workspace.package]
edition = "2021"
rust-version = "1.99.0"        # MSRV, kept equal to the toolchain pin
license = "GPL-3.0-only"
license-file = "LICENSE"
repository = "https://github.com/belikh/aquila"
authors = ["Aquila contributors"]

[workspace.lints.rust]
unsafe_code = "forbid"          # see §2.2
missing_docs = "warn"
rust_2018_idioms = { level = "warn", priority = -1 }
unused_qualifications = "warn"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
# Narrow, justified exceptions may be added here; see §4.2.
```

Every crate opts in with:

```toml
[lints]
workspace = true
```

and inherits shared package metadata with `version.workspace = true` (etc.).

### 2.1 Toolchain pin

`rust-toolchain.toml` is the single source of truth for the compiler. A toolchain
bump is a deliberate, single-purpose commit (never incidental) because
measurement determinism and the reference-pipeline bit-exactness asset
(`testing.md` §5) depend on a pinned toolchain and target triple.

```toml
[toolchain]
channel = "1.99.0"                 # exact pin; bump deliberately
components = ["rustfmt", "clippy"]
targets = ["wasm32-unknown-unknown", "x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"]
profile = "default"
```

`rust-version` in `[workspace.package]` is kept equal to `channel`, so the MSRV
claim and the CI toolchain cannot drift.

### 2.2 `unsafe` policy — `#![forbid(unsafe_code)]`

Unsafe Rust is forbidden workspace-wide through the workspace lint:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
```

`forbid` (stronger than `deny`) cannot be locally `allow`-ed and is inherited by
every crate that sets `[lints] workspace = true`. Expected count in v1 is **zero**
(`measurement.md` §4.3 emits `structure.unsafe.count` and expects 0 outside a
bounded FFI shim).

The one documented exception path, should it ever be required (e.g. a future
coherent-array FFI shim):

1. An ADR records the need and the exact scope.
2. That crate alone downgrades to `#![deny(unsafe_code)]` and uses a localised
   `#[allow(unsafe_code)]` with a `// SAFETY:` justification on each item.
3. The crate's public interface exposes no `unsafe`; the block is an
   implementation detail behind a safe API.

No other crate, and no test, may opt out. `aquila-dashboard`'s
`wasm-bindgen`-generated glue is not our `unsafe`; generated files are marked
`linguist-generated` in `.gitattributes` and excluded from the unsafe scan.

---

## 3. Crate inventory and dependency DAG

The crate graph is fixed by `docs/adr/0001-module-architecture.md` and
`docs/spec/architecture.md`: **eleven `aquila-*` modules** in six topological
levels, plus a composition root, a dev-only test crate, and repo tooling.

### 3.1 Levels (authoritative, derived topologically)

| Level | Crates | Character |
| --- | --- | --- |
| 0 | `aquila-types` | vocabulary; no behaviour |
| 1 | `aquila-signal`, `aquila-geometry`, `aquila-metrics` | pure transforms |
| 1 | `aquila-sources`, `aquila-inference` | ingestion and models (I/O behind seams) |
| 2 | `aquila-sensing` | the single stateful core |
| 3 | `aquila-evaluation`, `aquila-ha` | drive the core; HA output mapping |
| 4 | `aquila-api` | private control-plane transport |
| 5 | `aquila-dashboard` | WASM UI |

`aquila-sources` and `aquila-inference` are drawn on the same row as the pure
transforms because both depend only on `aquila-types`; they are not in the same
*dependency* relation to each other. The level **number** is what the check uses,
not the visual row.

> Inconsistency flagged: `architecture.md` §2 labels `aquila-api` as L4 and
> `aquila-ha` as L5, but §5's edges require `aquila-api → aquila-ha`. A module
> may depend only on a strictly lower level, so `aquila-ha` must sit **below**
> `aquila-api`. The edge allow-list is authoritative (ADR-0001 §Decision point
> 5); the level numbers above order the graph consistently and are what
> `tools/dag.toml` encodes. See §13.

### 3.2 The allow-list (authoritative edges)

Normal (non-dev, non-build) intra-workspace dependency edges, from
`architecture.md` §5:

```
aquila-types        -> ∅
aquila-signal       -> aquila-types
aquila-geometry     -> aquila-types
aquila-metrics      -> aquila-types
aquila-sources      -> aquila-types
aquila-inference    -> aquila-types
aquila-sensing      -> aquila-types, aquila-geometry, aquila-signal,
                       aquila-sources, aquila-inference
aquila-evaluation   -> aquila-types, aquila-sources, aquila-sensing, aquila-metrics
aquila-ha           -> aquila-types, aquila-geometry, aquila-sensing
aquila-api          -> aquila-types, aquila-geometry, aquila-sensing,
                       aquila-sources, aquila-ha
aquila-dashboard    -> aquila-api
```

Composition root and tooling (exempt from the level rule):

```
aquila-addon        -> any module above; holds no domain logic
aquila-testkit      -> any module above; depended on ONLY via [dev-dependencies]
xtask               -> may use any crate; never pulled into a product build
```

The eleven module crates are `publish = false`. The composition root and
`xtask` are `publish = false` binaries; `aquila-testkit` is `publish = false`.

### 3.3 The data — `tools/dag.toml`

The check is driven entirely by data, so a rename or a ratified new edge is a
one-file change reviewed against ADR-0001 — never a code change.

```toml
# Dependency-DAG allow-list. Single source of truth for allowed intra-workspace
# dependency edges. The check (xtask dag-check) fails on:
#   - any normal dep edge not listed here,
#   - any edge whose dependency is not in a strictly lower level,
#   - any cycle,
#   - any workspace member not registered below.
# Ratified by: docs/adr/0001-module-architecture.md
rev = 1

[levels]
"aquila-types"      = 0
"aquila-signal"     = 1
"aquila-geometry"   = 1
"aquila-metrics"    = 1
"aquila-sources"    = 1
"aquila-inference"  = 1
"aquila-sensing"    = 2
"aquila-evaluation" = 3
"aquila-ha"         = 3
"aquila-api"        = 4
"aquila-dashboard"  = 5

[exempt]
"aquila-addon"   = "composition root; may depend on any module"
"aquila-testkit" = "dev-only; never a normal dependency"
"xtask"          = "repo tooling; never a product dependency"

[edges]
"aquila-types"      = []
"aquila-signal"     = ["aquila-types"]
"aquila-geometry"   = ["aquila-types"]
"aquila-metrics"    = ["aquila-types"]
"aquila-sources"    = ["aquila-types"]
"aquila-inference"  = ["aquila-types"]
"aquila-sensing"    = ["aquila-types", "aquila-geometry", "aquila-signal",
                       "aquila-sources", "aquila-inference"]
"aquila-evaluation" = ["aquila-types", "aquila-sources", "aquila-sensing",
                       "aquila-metrics"]
"aquila-ha"         = ["aquila-types", "aquila-geometry", "aquila-sensing"]
"aquila-api"        = ["aquila-types", "aquila-geometry", "aquila-sensing",
                       "aquila-sources", "aquila-ha"]
"aquila-dashboard"  = ["aquila-api"]
```

---

## 4. Style and correctness tooling

### 4.1 rustfmt — `rustfmt.toml`

```toml
edition = "2021"
max_width = 100
use_small_heuristics = "Max"
group_imports = "StdExternalCrate"
imports_granularity = "Module"
newline_style = "Unix"
```

`cargo fmt --all --check` runs on every PR and gates merge. Only stable options
are used, so no nightly rustfmt is required.

### 4.2 clippy — `-D warnings`

CI runs:

```
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The workspace enables `clippy::all` and `clippy::pedantic` at `warn`; CI promotes
warnings to errors. Narrow, named exceptions live in `[workspace.lints.clippy]`
with a one-line reason above each, so the exception list is auditable. `clippy.toml`
carries only numeric thresholds:

```toml
too-many-arguments-threshold = 6
type-complexity-threshold = 250
```

### 4.3 cargo-deny — `deny.toml`

`cargo deny check advisories bans sources licenses` runs on every PR. The
workspace is `GPL-3.0-only`, so dependencies must be licence-compatible and
permissively licensed; the project licence does not force GPL on its runtime
dependencies, but strong copyleft dependencies are reviewed, not silently pulled.

```toml
[graph]
all-features = true

[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
ignore = []                    # an ignore requires a comment + expiry date

[licenses]
allow = [
    "MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib",
    "MPL-2.0", "Unicode-3.0", "CC0-1.0", "0BSD",
]
confidence-threshold = 0.93
# workspace crates are publish = false and are not licence-checked as deps

[bans]
multiple-versions = "warn"     # deny only after the tree is curated
wildcards = "deny"
deny = []                      # e.g. a crate with a known-bad licence/behaviour

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []                 # a git dependency requires an ADR + pin
```

### 4.4 cargo-machete

`cargo machete` runs on every PR to catch declared-but-unused dependencies
(including `[dev-dependencies]`). Genuine exceptions (a dependency used only via
a macro, a build script, or for a feature) get a per-package
`[package.metadata.cargo-machete] ignored = [...]` entry with a reason.

### 4.5 rustdoc tests

`cargo test --doc --workspace` runs on every PR. Rustdoc examples are tests
(`testing.md` §1.5): the interface is documentation.

---

## 5. The dependency-DAG guardrail

This is the key guardrail. Rust's crate boundaries give the *wall*; the DAG check
gives the *direction* (`architecture.md` §5).

### 5.1 Rule

For every normal (and build) dependency edge `A → B` where both `A` and `B` are
workspace members:

1. `B` must be a registered member of `tools/dag.toml` (in `[levels]` or `[exempt]`).
2. `B` must appear in `edges[A]` (or `A` must be in `[exempt]`).
3. If neither `A` nor `B` is exempt, `levels[B] < levels[A]` (strictly lower).
4. The whole workspace dependency graph must be acyclic.

Additional rules:

5. No module crate may declare a normal dependency on `aquila-testkit`; it may
   appear only under `[dev-dependencies]`. (`testing.md` §3.)
6. `xtask` and `aquila-addon` must not be normal dependencies of any other crate.

Dev- and build-dependency edges are also checked against rule 3 unless the
dependent is exempt; a dev-dependency on a same-or-higher level is a violation.

### 5.2 Implementation — `xtask dag-check`

The check is a first-class `xtask` subcommand and a `#[test]`, so it runs both as
`cargo xtask dag-check` (CI clarity) and under `cargo test --workspace` (hard to
skip):

```
xtask/
  Cargo.toml            # [dependencies] cargo_metadata, toml, anyhow
  src/lib.rs            # check_dag(manifest: &Path) -> Result<Report>
  src/dag.rs            # graph build + rule evaluation
  src/main.rs           # clap: dag-check | check-scaffolding | eval
  tests/dag.rs          # #[test] fn dag_is_respected() { ... }
```

Algorithm:

1. `cargo metadata --format-version 1 --no-deps` from the workspace root.
2. Load `tools/dag.toml`; parse `[levels]`, `[exempt]`, `[edges]`.
3. Build the graph from `resolve` (resolved deps) or, with `--no-deps`, from each
   member's `dependencies`/`dev-dependencies`/`build-dependencies` whose `path`
   is inside `crates/`.
4. Evaluate rules 1–6; on violation, fail with a message naming the offending
   edge, the two levels, and the ADR that owns the fix.
5. Detect cycles with a topological sort; a cycle is always fatal.

Failure output is actionable, e.g.:

```
DAG violation: aquila-signal -> aquila-sensing
  levels: aquila-signal=1, aquila-sensing=2 (must be strictly lower)
  not in tools/dag.toml [edges] for aquila-signal
  Owner: docs/adr/0001-module-architecture.md. A new edge requires an ADR.
```

### 5.3 What it protects

- Replay, simulation and evaluation stay runnable on fixtures with no hardware
  (only `aquila-sources` touches UDP; only `aquila-api` touches the network).
- `aquila-types` stays the shared alphabet, so the pure crates remain mutually
  independent.
- The WASM surface stays narrow: `aquila-dashboard → aquila-api` only.
- No accidental upward edge (e.g. `aquila-signal` reaching into `aquila-sensing`)
  and no cycle through the composition root.

The `structure.dag.violations` metric (`measurement.md` §4.3) records the count;
v1 expects 0.

---

## 6. Licensing and SPDX headers

Licence: **GPL-3.0-only** (SPDX `GPL-3.0-only`), matching `measurement.md` §Status.
The full text lives at `LICENSES/GPL-3.0-only.txt` (verbatim, unmodified);
`LICENSE` is a short project notice pointing at it.

Every source file carries an SPDX header in its native comment syntax. Exact
two-line form, first lines of the file:

- Rust (`*.rs`):
  ```rust
  // SPDX-FileCopyrightText: 2026 Aquila contributors
  // SPDX-License-Identifier: GPL-3.0-only
  ```
- C firmware (`firmware/**/*.{c,h}`):
  ```c
  /* SPDX-FileCopyrightText: 2026 Aquila contributors
   * SPDX-License-Identifier: GPL-3.0-only */
  ```
- Python (`ha/**/*.py`):
  ```python
  # SPDX-FileCopyrightText: 2026 Aquila contributors
  # SPDX-License-Identifier: GPL-3.0-only
  ```
- Shell (`*.sh`):
  ```sh
  # SPDX-FileCopyrightText: 2026 Aquila contributors
  # SPDX-License-Identifier: GPL-3.0-only
  ```

Files that cannot carry a header (JSON, fixtures, generated WASM glue, images)
are licensed in bulk by `REUSE.toml`:

```toml
version = 1
SPDX-PackageName = "aquila"
SPDX-PackageSupplier = "Aquila contributors"
SPDX-PackageDownloadLocation = "https://github.com/belikh/aquila"

[[annotations]]
path = ["docs/**", "*.md", "*.json", "*.yaml", "*.toml", "fixtures/**", "dashboard/pkg/**"]
precedence = "aggregate"
SPDX-FileCopyrightText = "2026 Aquila contributors"
SPDX-License-Identifier = "GPL-3.0-only"
```

`reuse lint` runs in CI and gates merge. Generated WASM glue is additionally
marked `linguist-generated=true` in `.gitattributes`.

---

## 7. Ignore files

### 7.1 `.gitignore` (must ignore agent scaffolding)

```gitignore
# ---- Agent scaffolding: never committed (see §8) ----
.claude/
.claude-flow/
.swarm/
CLAUDE.md
AGENTS.local.md

# ---- Rust ----
/target/
**/*.rs.bk
# Cargo.lock is committed (binaries/images are built from this workspace)
# Cargo.lock is NOT ignored

# ---- Firmware (ESP-IDF) ----
firmware/build/
firmware/sdkconfig.old
firmware/managed_components/
firmware/.pio/
firmware/**/*.o

# ---- Home Assistant component / add-on runtime ----
ha/**/__pycache__/
ha/**/*.pyc
ha/**/.venv/
ha/config/
ha/aquila_core/data/

# ---- Dashboard build ----
dashboard/node_modules/
dashboard/dist/
crates/aquila-dashboard/pkg/
crates/aquila-dashboard/target/

# ---- Runtime state (only the ledger is committed) ----
.aquila/*
!.aquila/metrics/
.aquila/metrics/*
!.aquila/metrics/ledger.jsonl
!.aquila/metrics/index.json
cache/

# ---- Local / secrets / OS ----
.env
*.local
*.pem
*.key
.DS_Store
*.swp
```

`Cargo.lock` is deliberately **committed**: the workspace ships binaries and
multi-arch container images, so builds must be reproducible from the lockfile.

### 7.2 `.gitattributes`

```gitattributes
* text=auto eol=lf
*.rs        text eol=lf
*.py        text eol=lf
*.sh        text eol=lf
Cargo.lock  text eol=lf
*.png       binary
*.wasm      binary
*.bin       binary
fixtures/** binary
crates/aquila-dashboard/pkg/** linguist-generated=true
```

### 7.3 `.editorconfig`

```ini
root = true
[*]
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true
indent_style = space
indent_size = 4
[*.{yml,yaml,json,toml}]
indent_size = 2
[Makefile]
indent_style = tab
```

---

## 8. No committed agent scaffolding

Policy: tools that generate agent scaffolding write it to ignored paths; **no
agent-generated harness, prompt, session, or tool tree is ever committed**. The
in-tree test doubles (`aquila-testkit`) are owned product code, not scaffolding
(`testing.md` §3, §10).

Two mechanisms enforce this:

1. `.gitignore` (§7.1) blocks the standard paths (`.claude`, `.claude-flow`,
   `.swarm`, `CLAUDE.md`).
2. A CI check fails if any such path is nonetheless **tracked**. The pattern list
   is data in `tools/no-agent-scaffolding.txt`:

   ```
   .claude/
   .claude-flow/
   .swarm/
   CLAUDE.md
   AGENTS.local.md
   .cursor/
   .aider*
   .continue/
   .specstory/
   ```

   The check (`xtask check-scaffolding`) runs `git ls-files -z` and fails,
   listing offending paths, if any tracked path matches a pattern. It never
   modifies the tree; it only reports.

This is a guardrail against accidental commits, not a moral stance on tooling:
the rule is "in-tree, human-reviewed artefacts only".

---

## 9. Git history, commits and releases

### 9.1 Conventional Commits

Every commit message (and the PR title, which becomes the merge commit subject)
follows **Conventional Commits 1.0.0**:

```
<type>(<scope>): <subject>

<body>

<footer>
```

- `type` ∈ `feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert`.
- `scope` is a crate or tree: `sensing`, `signal`, `metrics`, `api`, `ha`,
  `firmware`, `dashboard`, `repo`, `ci`, `docs`.
- Breaking changes use `!` after type/scope and a `BREAKING CHANGE:` footer.

Enforcement: `.github/workflows/commits.yml` runs `cog check` over the PR commit
range and checks the PR title against the same grammar. Nothing is linted over
the whole history — only new commits.

### 9.2 Non-squashed history

Individual commits are the unit of history; the repo preserves them.

- Repository setting: **Allow merge commits** and/or **Allow rebase merging**
  enabled; **Allow squash merging disabled**. Squash-merge destroys the
  per-commit history the ledger's commit-keying and `git diff` workflows rely on.
- Branch protection on `main`: require a PR, require the `build`, `test`,
  `commits` and `ledger` checks, require linear (non-squashed) merge, dismiss
  stale approvals, require branches up to date.
- Single-purpose commits; no "fix typo" on top of unrelated work; a revert uses
  `git revert` (a new commit), never a force-push rewrite of `main`.

### 9.3 Tags and releases

- Release tags are annotated `vMAJOR.MINOR.PATCH` (the add-on and component
  version independently; the crate versions follow semver per crate).
- Tags are signed (`git tag -s`) and immutable; a release is built from the tag
  by `package-addon.yml` (§10.4) and attested.
- Changelogs are derived from Conventional Commits (`cog changelog`), never
  hand-maintained in a way that can drift from history.

---

## 10. GitHub Actions workflows

All workflows follow a strict trust posture: third-party actions are pinned to a
full commit SHA with the tag in a trailing comment (updated by Dependabot);
default `permissions: contents: read`; no `pull_request_target` combined with
checkout of untrusted code; `persist-credentials: false` on checkout unless a
job pushes.

### 10.1 `build.yml` — format, lint, static checks, compile

```yaml
name: build
on:
  push: { branches: [main] }
  pull_request:
concurrency: { group: "build-${{ github.ref }}", cancel-in-progress: true }
permissions: { contents: read }
env:
  CARGO_TERM_COLOR: always
  CARGO_INCREMENTAL: "0"
  RUSTFLAGS: "-D warnings"

jobs:
  static:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@<pin>            # vX.Y.Z
        with: { persist-credentials: false }
      - uses: dtolnay/rust-toolchain@<pin>      # reads rust-toolchain.toml
      - uses: Swatinem/rust-cache@<pin>
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets --all-features -- -D warnings
      - run: cargo test --doc --workspace
      - name: Dependency DAG guardrail
        run: cargo xtask dag-check
      - name: No committed agent scaffolding
        run: cargo xtask check-scaffolding
      - uses: taiki-e/install-action@<pin>       # cargo-deny, cargo-machete
        with: { tool: "cargo-deny,cargo-machete" }
      - run: cargo deny check advisories bans sources licenses
      - run: cargo machete
      - uses: fsfe/reuse-action@<pin>            # reuse lint

  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@<pin>
        with: { persist-credentials: false }
      - uses: dtolnay/rust-toolchain@<pin>
      - uses: Swatinem/rust-cache@<pin>
      - run: cargo build --workspace --all-features --release
      - name: Dashboard WASM
        run: |
          rustup target add wasm32-unknown-unknown
          cargo install wasm-pack --locked
          wasm-pack build crates/aquila-dashboard --target web --release \
            --out-dir ../../dashboard/pkg
```

### 10.2 `test.yml` — tiers, coverage floor, reference pipeline, claims

Implements the CI topology in `docs/spec/testing.md` §8. Tiers 0–3, the coverage
floor, the reference-pipeline bit-exactness check and the claims verifier
**gate merge**; evaluation (tier 4) does **not**.

```yaml
name: test
on: [pull_request, workflow_dispatch]
concurrency: { group: "test-${{ github.ref }}", cancel-in-progress: true }
permissions: { contents: read }
env: { CARGO_TERM_COLOR: always }

jobs:
  tiers:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@<pin>
        with: { persist-credentials: false }
      - uses: dtolnay/rust-toolchain@<pin>
      - uses: Swatinem/rust-cache@<pin>
      - uses: taiki-e/install-action@<pin>
        with: { tool: "nextest,cargo-llvm-cov" }
      - run: cargo nextest run --workspace --all-features            # tiers 0-3
      - run: cargo llvm-cov --workspace --lcov --output-path lcov.info
      - name: Coverage floor
        run: cargo xtask coverage-floor --lcov lcov.info             # testing.md §6
      - name: Deterministic reference pipeline (bit-exact)
        run: cargo nextest run -p aquila-evaluation reference_pipeline_bit_exact
      - name: Claims verifier
        run: cargo xtask claims-verify                               # testing.md §7
```

### 10.3 `ledger.yml` — record + diff (no gate)

Runs on every PR and on `main`. It records measurements, appends the ledger on
`main`, and posts a diff. It **never fails on regressions**: `aquila-metrics diff`
exits 0 in v1 (`measurement.md` §6).

```yaml
name: ledger
on:
  push: { branches: [main] }
  pull_request:
  workflow_dispatch:
concurrency: { group: "ledger-${{ github.ref }}", cancel-in-progress: true }
permissions: { contents: read }

jobs:
  record-diff:
    runs-on: ubuntu-latest
    timeout-minutes: 45
    steps:
      - uses: actions/checkout@<pin>
        with: { fetch-depth: 0, persist-credentials: false }
      - uses: dtolnay/rust-toolchain@<pin>
      - uses: Swatinem/rust-cache@<pin>
      - name: Record measurements (replay + sim; tee to ledger and CI cache)
        run: cargo run -p aquila-metrics -- release --sink tee
      - name: Diff against baseline
        run: |
          BASE="${{ github.event.pull_request.base.sha || 'HEAD~1' }}"
          cargo run -p aquila-metrics -- diff "$BASE" HEAD \
            --format markdown --output ledger-diff.md
      - uses: actions/upload-artifact@<pin>
        with:
          name: measurement-ledger
          path: |
            .aquila/metrics/ledger.jsonl
            .aquila/metrics/index.json
            ledger-diff.md
      - name: PR comment
        if: github.event_name == 'pull_request'
        env: { GH_TOKEN: "${{ github.token }}" }
        run: gh pr comment "${{ github.event.pull_request.number }}" --body-file ledger-diff.md

  # On main only: append the ledger and commit it (non-squashed history kept).
  append:
    if: github.ref == 'refs/heads/main' && github.event_name == 'push'
    needs: record-diff
    runs-on: ubuntu-latest
    permissions: { contents: write }
    steps:
      - uses: actions/checkout@<pin>
        with: { persist-credentials: true, ref: main }
      - uses: actions/download-artifact@<pin>
        with: { name: measurement-ledger, path: .aquila/metrics }
      - name: Commit the appended ledger
        run: |
          git config user.name  "github-actions[bot]"
          git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
          git add .aquila/metrics/ledger.jsonl .aquila/metrics/index.json
          git diff --cached --quiet || git commit -m "chore(metrics): append ledger for ${GITHUB_SHA} [skip ci]"
          git push
```

The `[skip ci]` marker and `paths-ignore` guard against a ledger-commit loop.

### 10.4 `package-addon.yml` — build and publish the add-on and HACS asset

Runs on `v*` tags. Builds the composition root for both supported arches
(`amd64`, `aarch64`; `armv7`/`armhf` unsupported by decision —
`home-assistant.md` §3.1), bundles the WASM dashboard, and pushes multi-arch
images to GHCR.

```yaml
name: package-addon
on:
  push: { tags: ["v*"] }
  workflow_dispatch:
permissions:
  contents: write        # release assets
  packages: write        # ghcr.io
jobs:
  package:
    runs-on: ubuntu-latest
    strategy:
      matrix: { arch: [amd64, aarch64] }
    steps:
      - uses: actions/checkout@<pin>
        with: { fetch-depth: 0 }
      - uses: dtolnay/rust-toolchain@<pin>
      - uses: docker/setup-qemu-action@<pin>
      - uses: docker/setup-buildx-action@<pin>
      - uses: docker/login-action@<pin>
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - name: Build dashboard WASM
        run: |
          rustup target add wasm32-unknown-unknown
          cargo install wasm-pack --locked
          wasm-pack build crates/aquila-dashboard --target web --release \
            --out-dir ../../dashboard/pkg
      - name: Build and push add-on image
        uses: docker/build-push-action@<pin>
        with:
          context: .
          file: ha/aquila_core/Dockerfile
          platforms: ${{ matrix.arch == 'amd64' && 'linux/amd64' || 'linux/arm64' }}
          push: true
          tags: ghcr.io/belikh/aquila-core-${{ matrix.arch }}:${{ github.ref_name }}
      - name: Package HACS component asset (aquila.zip)
        run: |
          cd ha && zip -r ../aquila.zip custom_components/aquila
      - name: Attest build provenance
        uses: actions/attest-build-provenance@<pin>
        with: { subject-path: "aquila.zip" }
      - uses: softprops/action-gh-release@<pin>
        with: { files: aquila.zip }
```

Release invariants (home-assistant.md §9): the add-on bumps `version:` in
`ha/aquila_core/config.yaml`; the component bumps `version` in
`ha/custom_components/aquila/manifest.json`; the two version independently and
meet at the versioned private API, not at a shared version number.

### 10.5 `commits.yml` — Conventional Commits

```yaml
name: commits
on: [pull_request]
permissions: { contents: read, pull-requests: read }
jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@<pin>
        with: { fetch-depth: 0, persist-credentials: false }
      - uses: cocogitto/cocogitto-action@<pin>
        with: { command: "check", args: "${{ github.event.pull_request.base.sha }}..${{ github.event.pull_request.head.sha }}" }
      - uses: amannn/action-semantic-pull-request@<pin>
        env: { GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }} }
```

### 10.6 `nightly.yml` — non-gating, long-running

Runs on a schedule; never gates a PR (`testing.md` §8).

- `cargo-mutants` over the core crates (`aquila-signal`, `aquila-sensing`,
  `aquila-inference`) — survivors triaged as a real gap or a documented
  equivalent mutant.
- Simulator soak and external-fixture fetch (MM-Fi, licence-gated: fetched on
  demand against a pinned digest, never redistributed — `testing.md` §4).
- Evaluation (tier 4): real models over a committed eval split; records ledger
  rows, gates nothing.

---

## 11. Guardrail checks (the list)

Every item below is a check that fails the build or the merge. This is the
authoritative guardrail list for the repository.

| # | Guardrail | Mechanism | Fails |
| --- | --- | --- | --- |
| G01 | Formatting | `cargo fmt --all -- --check` | `build.yml` |
| G02 | Lints | `cargo clippy … -D warnings` (workspace `all` + `pedantic`) | `build.yml` |
| G03 | **Dependency DAG: no upward import** | `cargo xtask dag-check` over `cargo metadata` vs `tools/dag.toml` | `build.yml` |
| G04 | **No cycle** in the workspace graph | same check (topological sort) | `build.yml` |
| G05 | **Every member registered** in `tools/dag.toml` | same check | `build.yml` |
| G06 | **`aquila-testkit` is dev-dependency only** | same check (rule 5) | `build.yml` |
| G07 | **No committed agent scaffolding** | `cargo xtask check-scaffolding` over `git ls-files` vs `tools/no-agent-scaffolding.txt` | `build.yml` |
| G08 | **`unsafe` forbidden** | `[workspace.lints.rust] unsafe_code = "forbid"` + `structure.unsafe.count` | `build.yml`, ledger |
| G09 | Licence compliance | `cargo deny check licenses` | `build.yml` |
| G10 | Advisories | `cargo deny check advisories` | `build.yml` |
| G11 | Banned/wildcard deps | `cargo deny check bans` | `build.yml` |
| G12 | Dependency sources | `cargo deny check sources` (crates.io only) | `build.yml` |
| G13 | Unused dependencies | `cargo machete` | `build.yml` |
| G14 | SPDX / licence headers | `reuse lint` | `build.yml` |
| G15 | Doc examples compile | `cargo test --doc --workspace` | `build.yml` |
| G16 | Build (all features, all targets) | `cargo build --workspace --all-features` | `build.yml` |
| G17 | WASM build | `wasm-pack build crates/aquila-dashboard` | `build.yml` |
| G18 | Tests, tiers 0–3 | `cargo nextest run --workspace` | `test.yml` |
| G19 | Coverage floor | `cargo xtask coverage-floor` (`testing.md` §6) | `test.yml` |
| G20 | Reference pipeline bit-exactness | canonical-target nextest | `test.yml` |
| G21 | Honest-claims bijection | `cargo xtask claims-verify` (`testing.md` §7) | `test.yml` |
| G22 | Conventional Commits | `cog check <range>` + PR title | `commits.yml` |
| G23 | Non-squashed history | branch protection (squash-merge disabled) | repo policy |
| G24 | Measurement recorded + diffed (**no gate**) | `aquila-metrics record` / `diff` (exit 0) | never |

G03–G08 and G23–G24 are the checks specific to this document; the rest implement
the tooling and the testing strategy named elsewhere. Measurement (G24) is
deliberately **not** a gate: record and diff only, v1 (`measurement.md` §5).

---

## 12. Determinism and reproducibility

- `Cargo.lock` is committed; CI builds with `--locked` where a lockfile-changing
  step is not intended.
- The toolchain is pinned; a bump is its own reviewed commit.
- The ledger is canonical (sorted keys, Ryu float formatting, LF,
  `measurement.md` §3); golden-ledger fixtures are re-serialised in tests.
- Container builds use `docker buildx` with the pinned base image digest in
  `ha/aquila_core/Dockerfile`, and release artefacts are attested
  (`actions/attest-build-provenance`).
- The reference-pipeline bit-exactness asset is regenerated only by an explicit,
  reviewed act (`testing.md` §5).

---

## 13. Open questions and reconciliations

Resolved by this document, flagged for ADR-0001/measurement owner follow-up:

1. **`architecture.md` §2 layer labels vs §5 edges.** §2 lists `aquila-api` at L4
   and `aquila-ha` at L5, but §5 requires `aquila-api → aquila-ha`. This document
   derives levels from the §5 edge allow-list (ha below api) and encodes them in
   `tools/dag.toml`. Follow-up: correct §2, or bless the derived ordering in
   ADR-0001.
2. **Ledger path.** `measurement.md` says `.aquila/metrics/ledger.jsonl`;
   `data-contracts.md` §9.2 says `measure/ledger.jsonl`. This document follows
   the ledger's owning crate and uses `.aquila/metrics/`.
3. **HA tree location.** `architecture.md` §9 (`addon/`, root
   `custom_components/`) and `home-assistant.md` §3.3 (`aquila_core/`, root
   `custom_components/`) are reconciled to the brief's `ha/` tree here (§1.2).
   HACS `zip_release` makes the repository path independent of the HA runtime
   path.

Open, not blocking v1:

- Whether `aquila-testkit` remains (per `testing.md` §3) or its fakes move into
  the owning crates as adapters (per `architecture.md` §6). Either is DAG-legal;
  the check handles both.
- Exact coverage-floor tooling and whether `cargo-llvm-cov` region coverage is
  the metric or line coverage (tie to `testing.md` §6).
- Whether third-party actions move to a vendored runner image if the pinned-SHA
  maintenance cost proves high.
