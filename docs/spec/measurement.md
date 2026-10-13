# Measurement & ledger

**Status:** settled spec. Crate: `aquila-metrics` (binary `aquila-metrics`).
**Licence:** GPL-3.0-only. **Spelling:** Australian English.

Aquila treats measurement as a first-class module, not a dashboard bolted on.
Every claim Aquila makes about itself — correctness, determinism, structure,
performance, accuracy — is a **Measurement** written to a commit-keyed ledger
through a single seam, and compared run-to-run by a `diff`. In v1 the ledger
**records and diffs only; nothing hard-gates CI**.

---

## 1. Purpose and non-goals

**Purpose**

- Give every self-claim a typed, reproducible, commit-keyed record.
- Make the difference between two commits legible in one command.
- Keep the honesty contract enforceable later without pretending it is enforced now.

**Non-goals (v1)**

- No CI gating. Recorded and surfaced, never blocking.
- No public REST/WS/MCP surface. The ledger is a local artefact plus a CLI.
- No dashboard-owned metrics. The management dashboard may *render* the ledger;
  it does not own the numbers.
- No Prometheus in v1. The adapter is specified and stubbed only.

**Seams consumed**

- `Clock` — supplies `timestamp`; the module never calls `SystemTime::now()`.
- `MetricSink` — the one write path out of the module (this spec).

`aquila-metrics` sits near the bottom of the dependency DAG. It depends on the
seam traits and a serialisation library; nothing in the DSP, inference, fusion or
calibration graph depends on it. Tests, benches, the add-on and CI tooling depend
on it — never the reverse.

---

## 2. The `Measurement` type

One observation. Flattened into the ledger; the identity of a record is derived,
not stored.

```rust
/// A single, commit-keyed observation recorded by the measurement module.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    /// Stable metric name, e.g. `accuracy.presence_f1`, `micro.instr.ingest.parse`.
    pub name: MetricName,
    /// Definition version; bumped whenever the metric's meaning or method changes.
    pub version: u32,
    /// The observed quantity, tagged so digests and counts are never coerced to f64.
    pub value: MeasurementValue,
    /// Unit of `value`: "%", "count", "ms", "brpm", "bytes", "ratio", "sha256", "score".
    pub unit: Unit,
    /// Observations aggregated into `value` (1 for a single scalar run; N for means).
    pub samples: u32,
    /// Master RNG / fixture seed, or `None` when the metric is seedless.
    pub seed: Option<u64>,
    /// Exact commit the measurement was taken at (full 40-hex SHA).
    pub commit: CommitSha,
    /// True when the working tree had uncommitted changes at record time.
    pub dirty: bool,
    /// Stable digest of host identity (CPU, cores, arch, OS, runner class).
    pub host: HostFingerprint,
    /// Tool name → version for every tool that produced or could influence the value.
    pub tools: ToolVersions,
    /// Provenance of the number: where the evidence came from.
    pub authority: Authority,
    /// Epistemic class that governs diffing: how much a change means.
    pub grade: Grade,
    /// UTC measurement time, taken from the `Clock` seam.
    pub timestamp: Timestamp,
}

/// Value is tagged, never a bare f64, so `Digest` and `Count` stay exact.
pub enum MeasurementValue {
    /// Continuous quantities: ms, %, brpm, ratio, score.
    Scalar(f64),
    /// Integral counts: tests, DAG violations, bytes, unsafe blocks.
    Count(u64),
    /// Content hashes: reference-pipeline SHA-256, artefact/model/eval digests.
    Digest(String),
    /// Booleans: e.g. DAG-clean, unsafe-forbidden holds.
    Flag(bool),
}

/// Provenance: the strength of the evidence behind this number.
pub enum Authority {
    /// Executed on this host, now.
    Measured,
    /// Produced from committed recorded fixtures / replay (sim-first).
    Replayed,
    /// Computed from other measurements (e.g. mutation score = killed / total).
    Derived,
    /// Brought in from an external system (OTel, CI cache restore).
    Imported,
    /// Hand-entered baseline or a research-only ground truth (camera-GT, ESP32-as-SDR).
    Declared,
}

/// Class that governs how `diff` reads a change.
pub enum Grade {
    /// Same inputs ⇒ byte-identical value on a pinned toolchain.
    Deterministic,
    /// Run-to-run variance; compared as a distribution, never asserted exactly.
    Statistical,
    /// Statistical in nature, plus an explicit threshold bar (v1: accuracy bars only).
    Gated,
}
```

### Field notes

- **`name`** carries the dimension. Percentiles, stages and benches are encoded
  in the name (`system.latency.p99.ingest`, `micro.instr.ingest.parse`), so no
  separate tag map is needed and names sort canonically.
- **`version`** is essential: a metric's *definition* can change (tolerance,
  method, split). Diffing across a version change is reported as
  `uncomparable`, never as a regression.
- **`value`** is tagged so a SHA-256 or a byte count is never lossily squeezed
  through `f64`.
- **`seed`** is a single master seed; per-run seeds derive from it
  deterministically and are recorded in `tools` as a seed-set digest.
- **`commit` + `dirty`**: a dirty tree means the SHA does not pin the code. A
  dirty `head` downgrades the whole diff's authority and must be shown loudly.
- **`host`**: micro- and system-perf are host-dependent. Host-sensitive
  comparisons across differing fingerprints are reported `host-mismatch` and
  downgraded to informational (see §6).
- **`tools`** is a sorted name→version map (rustc, cargo, cargo-llvm-cov,
  cargo-mutants, iai-callgrind/valgrind, criterion, node, wasm-pack,
  onnxruntime, the eval-split digest, the model digest). Pin everything that can
  move the number.
- **`authority`** keeps simulated, replayed and research-only numbers visibly
  distinct from measured ones. Honest claims only.
- **`grade`** is the diff semantics; it is orthogonal to `authority`
  (`Replayed` accuracy can still be `Gated`; `Measured` latency is `Statistical`).
- **`timestamp`** is injected from `Clock`, so tests are deterministic.

### Identity and dedup key

A record's canonical key is
`(name, version, commit, host, seed)`. The ledger is append-only:

- **Deterministic** metrics: a repeat with the same key must yield an identical
  value; a mismatch is a recorded **contradiction**, not a silent overwrite.
- **Statistical / gated** metrics: repeats are retained and aggregated;
  `samples` counts them and the stored `value` is the declared aggregate (mean or
  percentile, per the metric definition).

`id` is `sha256` of the canonically serialised record *excluding* `timestamp`,
so re-recording a statistical sample does not mint a new identity, and the
timestamp never participates in comparisons.

---

## 3. The `MetricSink` seam

Deep module, small interface: **write-only**. Reading and querying live in the
ledger store, never behind the sink.

```rust
pub trait MetricSink: Send {
    fn record(&mut self, m: &Measurement) -> Result<(), MetricsError>;
    /// Optional; buffered adapters flush here. Default is a no-op.
    fn flush(&mut self) -> Result<(), MetricsError> { Ok(()) }
}
```

### Adapters

| Adapter | v1 | Behaviour |
| --- | --- | --- |
| `JsonLedgerSink` | yes | Appends canonical JSONL to the commit-keyed ledger; the source of truth for `diff`. |
| `CiCacheSink` | yes | Writes the canonical set under `cache/<commit>/<host>/`, keyed so CI can restore a baseline without re-running; restores as `Authority::Imported`. |
| `PrometheusSink` | later | Maps gauges/histograms to an OpenMetrics push; only statistical/runtime metrics; specified and stubbed, not wired in v1. |
| `MemorySink` | yes (fake) | Collects into a `Vec<Measurement>` for TDD assertions. |
| `NullSink` | yes (fake) | Discards; for benches that must not touch disk. |
| `TeeSink` | yes | Fans one record out to several sinks; fails closed if any required sink fails. |

`TeeSink` is how a run writes both the local ledger and the CI cache without the
caller branching.

### Ledger layout and canonicalisation

```
.aquila/
  metrics/
    ledger.jsonl      # append-only; one canonically serialised Measurement per line
    index.json        # commit -> record ids (derived, rebuildable, never authoritative)
    baseline.json     # optional pinned baseline selection for `diff`
```

- Canonical ordering: sort by `(commit, name, version, host, seed)`.
- Canonical serialisation: sorted `tools` keys; shortest round-trip float
  formatting (Ryu); no insignificant whitespace. The ledger is itself diffable.
- `index.json` is a cache; deleting it changes nothing.

### Testing (TDD, fake adapters, recorded fixtures)

- `FixedClock` injects a fixed timestamp; `MemorySink` captures records.
- Golden ledger fixtures are committed and re-serialised in tests to prove
  canonical form is stable.
- Every metric source is exercised through a recorded fixture first (replay), so
  the module and its metrics work without hardware. No committed agent
  scaffolding; fakes live in the crate, not in a parallel tree.

---

## 4. Metric catalogue

**How to read `grade`**

- **Deterministic** — pinned inputs and toolchain give a byte-identical value. A
  change is a *signal*, surfaced first, and must be explained. Never a flake.
- **Statistical** — run-to-run variance. Compared to a tolerance band; reported,
  never asserted exactly.
- **Gated** — statistical in nature *and* carrying an explicit threshold bar.
  Its pass/fail is recorded and shown, but advisory in v1. In v1 the only gated
  metrics are the three accuracy bars.

### 4.1 Correctness

| Metric | Unit | Grade | Authority | Source |
| --- | --- | --- | --- | --- |
| `correctness.tests.total` | count | deterministic | measured | `cargo nextest` |
| `correctness.tests.failed` | count | deterministic | measured | `cargo nextest` |
| `correctness.tests.flaky` | count | statistical | derived | reruns (nextest `--retries`) |
| `correctness.coverage.lines` | % | deterministic | measured | `cargo-llvm-cov` |
| `correctness.coverage.branches` | % | deterministic | measured | `cargo-llvm-cov` |
| `correctness.mutation.score` | ratio | deterministic | derived | `cargo-mutants` (killed/total) |

Flakiness is the one genuinely statistical correctness signal and is tracked
separately so it never masks a real failure. Mutation testing is deterministic
given a pinned mutant set and tool version; it is slow and runs nightly.

### 4.2 Determinism

| Metric | Unit | Grade | Authority | Source |
| --- | --- | --- | --- | --- |
| `determinism.pipeline.sha256` | sha256 | deterministic | replayed | reference pipeline over the committed fixture corpus |
| `determinism.stage.<stage>.sha256` | sha256 | deterministic | replayed | per-stage intermediate digest (optional) |

The **reference-pipeline SHA-256** is the anchor: the recorded fixtures are run
through ingest → DSP → feature → fusion, and the output is hashed. Any change in
the digest means the pipeline changed and must be explained or the fixtures
re-baselined deliberately. Diff rule: digest equality; a difference is an event.

### 4.3 Structure

All deterministic, all `Measured` from static analysis of the workspace.

| Metric | Unit | Grade | Source |
| --- | --- | --- | --- |
| `structure.crates.count` | count | deterministic | `cargo metadata` |
| `structure.dag.violations` | count | deterministic | `cargo metadata` vs committed allowlist `aquila-dag.toml` |
| `structure.types.duplicate` | count | deterministic | in-tree `syn` scan for structurally identical/near-identical type defs |
| `structure.docs.ratio` | ratio | deterministic | in-tree counter (doc-comment lines : code lines), per crate and workspace |
| `structure.unsafe.count` | count | deterministic | in-tree `syn` scan for `unsafe` blocks / `unsafe fn` |

The counter and scan tools are in-tree so versions and results are pinned and
dependency-free. `structure.unsafe.count` is expected to be 0 outside a bounded
FFI shim.

### 4.4 Micro-performance

| Metric | Unit | Grade | Authority | Source |
| --- | --- | --- | --- | --- |
| `micro.instr.<bench>` | count | deterministic | measured | `iai-callgrind` (Ir instructions) |
| `micro.time.<bench>` | ms | statistical | measured | `criterion` |

Instruction counts are the deterministic anchor (cachegrind Ir is stable for a
pinned binary on a pinned host); wall-clock criterion numbers are statistical and
advisory. Both are **host-sensitive** and carry the host fingerprint.

### 4.5 System-performance

| Metric | Unit | Grade | Authority | Source |
| --- | --- | --- | --- | --- |
| `system.latency.p50.<stage>` | ms | statistical | measured | end-to-end replay soak |
| `system.latency.p95.<stage>` | ms | statistical | measured | end-to-end replay soak |
| `system.latency.p99.<stage>` | ms | statistical | measured | end-to-end replay soak |
| `system.frames.per_second` | count | statistical | measured | ingest throughput harness |
| `system.rss.peak` | bytes | statistical | measured | soak harness (`getrusage`/`/proc`) |
| `system.binary.size.<profile>` | bytes | deterministic | measured | pinned toolchain build |
| `system.wasm.size.dashboard` | bytes | deterministic | measured | pinned `wasm-pack` build |

Latency percentiles, throughput and RSS are statistical (allocator and scheduling
dependent). Binary and WASM size are deterministic for a pinned toolchain; a
change is reported with its tool versions attached.

### 4.6 Accuracy (gated)

Evaluated by ONNX inference over a committed eval split; both arms recorded:
MM-Fi and own captures. Model digest, eval-split digest and seed are recorded in
`tools`. Authority is `Replayed` for fixture/replay evaluation.

| Metric | Unit | Grade | Bar (v1) | Direction |
| --- | --- | --- | --- | --- |
| `accuracy.torso_pck_at_20` | % | **gated** | ≥ 65 | higher is better |
| `accuracy.presence_f1` | score | **gated** | ≥ 0.9 | higher is better |
| `accuracy.breathing.mae_brpm` | brpm | **gated** | ≤ 2 | lower is better |
| `accuracy.heart_rate.mae_bpm` | bpm | statistical | — | non-gated in v1 |

Only these three bars are gated. Research-only sources (camera ground truth,
ESP32-as-SDR) are `Declared` and never gated. Honest claims only: a passing bar
is reported as *met on this split and seed*, not as a general guarantee.

### 4.7 Runtime (OpenTelemetry)

| Metric | Unit | Grade | Authority | Source |
| --- | --- | --- | --- | --- |
| `runtime.<counter\|histogram>` | varies | statistical | imported | OpenTelemetry from the add-on |

Runtime metrics are recorded opportunistically from the running add-on and are
**not** part of the offline ledger or the CI diff in v1. They reach the ledger
later through `PrometheusSink`; until then they are `Imported` and excluded from
`diff` unless explicitly requested.

### 4.8 Catalogue summary

| Family | Grade(s) | Gated in v1 | In CI diff |
| --- | --- | --- | --- |
| Correctness | deterministic (+ flaky statistical) | no | yes |
| Determinism | deterministic | no | yes |
| Structure | deterministic | no | yes |
| Micro-performance | deterministic + statistical | no | yes |
| System-performance | statistical + deterministic (sizes) | no | yes |
| Accuracy | gated (+ statistical) | yes (3 bars, advisory) | yes |
| Runtime | statistical | no | no (imported later) |

---

## 5. Policy: record + diff, nothing hard-gates CI

1. **Record everything.** Every catalogue metric that a run can produce is
   written to the ledger unconditionally — passing, failing or regressed.
2. **Diff everything comparable.** `diff` reports every metric present in either
   side, including additions and removals.
3. **Gate nothing.** No metric blocks a merge or a build in v1. Gated metrics
   report pass/fail and margin; the command still exits 0.
4. **Regressions are surfaced, not silenced.** Deterministic changes and gated
   failures are ordered first and highlighted, but remain advisory.
5. **Honesty guards.** A dirty `head`, a `host-mismatch`, or a metric `version`
   change downgrades the comparison and is labelled, never hidden.

A `--strict` flag is reserved for a future, separately-decided gating step. It is
**off and unsupported in v1**, and enabling it is an explicit policy change, not
a config toggle.

---

## 6. The `aquila-metrics diff` command

```
aquila-metrics diff <baseline> <head>
    [--format markdown|json|text]   # default: text
    [--output <path>]               # write the report; stdout if omitted
    [--metric <glob>]               # restrict by metric name
    [--class deterministic|statistical|gated]
    [--tolerance <pct>]             # override the statistical band
    [--strict]                      # RESERVED — reserved and unsupported in v1
```

- `<baseline>` / `<head>` are commit-ish selectors (`<sha>`, short sha, `HEAD`,
  `HEAD~1`) or a ledger/file path. Each resolves to a commit-keyed measurement set.
- If a metric is absent on one side, the verdict is `added` / `removed`, never a
  silent zero.
- **Deterministic**: verdict `changed` / `unchanged`; for `Digest`, equality.
- **Statistical**: verdict `within-tolerance` / `outside-tolerance` against the
  metric's declared band (default ±5 %, overridable).
- **Gated**: verdict `gate-pass` / `gate-fail`, reported with the value, the bar
  and the margin, in the correct direction.
- **`host-mismatch`**: when baseline and head host fingerprints differ and the
  metric is host-sensitive (micro-/system-perf), the verdict is
  `host-mismatch` and the number is informational only.
- **`uncomparable`**: when `version`, unit or grade differ between sides.

**Exit code:** `0` on any successful comparison in v1, even with regressions or
gate failures. A non-zero exit is reserved for future `--strict` and is not
implemented now. Command *errors* (missing ledger, unreadable path) exit non-zero.

```
aquila-metrics record [--sink json|ci|tee] [--selector <glob>]
aquila-metrics list   [--commit <sha>] [--class ...]
aquila-metrics diff   <baseline> <head>
```

---

## 7. The PR comment

Posting the comment is a thin CI wrapper over `diff --format markdown`; the module
owns the content, not the transport.

```markdown
## Aquila measurements — `a1b2c3d` ⟷ `e4f5g6h`

> ⚠️ head tree is **dirty** — the SHA does not pin the code.
> Host: `linux-x86_64-8c32` (baseline `linux-x86_64-8c32`) — micro-perf comparable.

### Gated (advisory — nothing blocks CI yet)
| Metric | Baseline | Head | Bar | Verdict |
|---|---|---|---|---|
| accuracy.presence_f1 | 0.912 | 0.898 | ≥ 0.90 | ❌ miss (margin −0.002) |
| accuracy.breathing.mae_brpm | 1.7 | 1.6 | ≤ 2 | ✅ pass |
| accuracy.torso_pck_at_20 | 66.1 | 67.4 | ≥ 65 | ✅ pass |

### Deterministic changes (explain each)
| Metric | Baseline | Head | Δ |
|---|---|---|---|
| determinism.pipeline.sha256 | 9f2c… | 7a01… | changed |
| structure.unsafe.count | 0 | 0 | — |

### Statistical (tolerance ±5%)
| Metric | Baseline | Head | Δ | Verdict |
|---|---|---|---|---|
| system.latency.p99.ingest | 41.2 ms | 43.0 ms | +4.4% | within-tolerance |
| micro.instr.ingest.parse | 812k | 809k | −0.4% | improved |

<details><summary>Full catalogue (34 metrics)</summary>

… one row per metric, grouped by family …

</details>

_Recorded and diffed only. No metric gates CI in v1. Reproduce:
`aquila-metrics diff a1b2c3d e4f5g6h`._
```

The comment always states the dirty flag, the host fingerprints, and the
no-gating posture; it renders the machine-readable JSON alongside when a bot
consumes it.

---

## 8. Placement and enforcement

- **Dependency DAG:** `aquila-metrics` → { seam traits, serialisation }. Nothing
  upstream depends on it. `structure.dag.violations` checks this edge exists and
  that no DSP/inference crate gains a dependency back onto it.
- **Determinism of the module itself:** all host I/O enters through `Clock` and
  `MetricSink`; the module is pure given those. The module is covered by
  `determinism.pipeline.sha256` only indirectly — its own tests use `FixedClock`
  and `MemorySink` and golden fixtures.
- **No committed agent scaffolding:** fakes and fixtures live inside the crate,
  not in a parallel tree; there is no generated harness committed to the repo.

---

## 9. Honesty constraints

- A passing gate means *met on this split, seed and tool version* — never a
  general claim.
- Cross-host numbers are not comparable; `diff` says so rather than implying it.
- A dirty tree invalidates commit-keying; the report says so rather than implying it.
- Replayed, simulated and research-only (`Declared`) numbers are labelled and never
  gated.
- Metric definition changes make two sides `uncomparable`; they are never plotted
  as a regression.

---

## 10. Open questions (deferred, not blocking)

- Exact tolerance bands per statistical metric (start at ±5 %, calibrate later).
- Whether `structure.types.duplicate` should be exact-structural or fuzzy.
- Whether runtime OTel metrics ever enter the offline `diff` or stay dashboard-side.
- The eventual `--strict` policy: which deterministic changes, if any, may ever
  block — a deliberate, separately-decided step.
