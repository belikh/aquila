# Testing & TDD — Aquila

Status: settled v1 strategy. Owner: testing & TDD.
Scope: the sensing core, the calibration/fusion/inference modules, the
`aquila-testkit` crate, fixtures, the reference pipeline, coverage/mutation
budgets, and the honest-claims contract. Does not define the DSP algorithms
themselves (see the measurement and sensing specs) or the product claims
themselves (see `README.md`).

Aquila is a measurably honest pipeline. Tests are how that honesty is enforced:
the suite is the mechanical counterpart of the README's claims, and every claim
is either backed by a recorded value or explicitly labelled as a target.

## 1. The interface is the test surface

> **Rule.** A test touches only what an interface exposes. The unit of test is a
> deep module's public interface; everything the module needs from the outside
> arrives through a real seam. Tests never reach past the interface into private
> types, private fields, or internal functions.

The real seams are the six named adapters: `CsiSource`, `Clock`,
`PoseModel`/`VitalsModel`, `GroundTruth`, `Sink`, `MetricSink`. A deep module
hides a large implementation behind a small interface; the interface is the
entire contract, and the test suite is written against it.

Corollaries, all binding:

1. **Fakes are adapters, not mocks.** A fake implements the *same trait* as the
   production adapter. There is no separate "test shape" of an interface. A
   shared contract-test suite runs against every implementation of a trait —
   the fake, the simulator, and (where it can be run locally) the real adapter —
   so a fake cannot silently drift from the contract.
2. **No test-only back doors.** `#[cfg(test)]` may add fixtures and helpers; it
   must never introduce an alternate code path into production logic. There are
   no test-only accessors or `pub(crate)` escapes created for a test's benefit.
3. **If it cannot be asserted through the interface, the interface is at
   fault.** Extend the interface (or add a declared seam) rather than reaching
   in. A test that needs to observe an internal is evidence of a missing seam.
4. **TDD.** The failing test is the interface's first consumer. Writing it first
   forces the interface to exist and to be sufficient before any implementation
   exists. Test names read as behaviour, not as method names.
5. **The interface is documentation.** Rustdoc examples are tests and run in CI.

Because I/O lives only at the edges (adapters), core unit tests are pure,
in-process, and fast; determinism is not an accident but a property of the
design.

## 2. Test tiers

Five tiers, each named by what it is allowed to prove. Correctness tiers
(0–3) plus the claims verifier and the reference pipeline gate merge. The
accuracy tier (4) does not gate; it records and diffs.

| Tier | Name | What it tests | Collaborators | Determinism | Budget | CI role |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | Unit | One core module through its public interface | Fake adapters from `aquila-testkit` | Total | < 100 ms/test | PR; gates merge |
| 1 | Conformance | A module or the pipeline against a recorded/golden fixture | Real core code, `FakeClock`, extra fakes | Bit-exact on the canonical target | < 2 s/fixture | PR; gates merge |
| 2 | Property | DSP invariants over generated inputs | `DeterministicCsiGenerator` + `FakeClock` | Seeded and reproducible | < 10 s/suite | PR; gates merge |
| 3 | Integration | The whole pipeline via the multi-node, three-floor simulator | `Simulator`, fake models/clock | Seeded and reproducible | < 60 s | PR; gates merge |
| 4 | Evaluation | Accuracy vs ground truth (PCK@20, F1, breathing MAE) | Real models, `recorded`/`derived` fixtures, `GroundTruth` | Statistical, multiple seeds | Minutes–hours | Out-of-CI; records to ledger, no gate |

Tier 1 also covers **adapter conformance**: the real node UDP client and the HA
ingress replay recorded protocol captures, checked against the same contract
suites the fakes satisfy.

The `aquila-testkit` crate provides the fakes and generator used across all
tiers.

## 3. aquila-testkit

`aquila-testkit` is first-class, in-tree product infrastructure — **not agent
scaffolding**. It is a `dev-dependency` (`publish = false`) with its own tests
and its own coverage floor. Changes to the generator are a breaking change:
fixtures must be regenerated and their provenance bumped.

| Component | Seam | Behaviour |
| --- | --- | --- |
| `InMemoryCsiSource` | `CsiSource` | Streams a supplied frame vector, or a generator; pacing is driven by the fake clock. Supports "replay a fixture" and "drive a generator". |
| `FakeClock` | `Clock` | Manually advanced time: fixed step, jump, or scripted schedule. No wall-clock anywhere in core tests. |
| `FakePoseModel` / `FakeVitalsModel` | `PoseModel` / `VitalsModel` | Deterministic stub models: identity, constant, or table-driven. Pipeline, calibration and fusion tests never depend on ONNX. |
| `DeterministicCsiGenerator` | producer for `CsiSource` | Seeded CSI synthesiser. Given seed + scene spec (nodes, floors, rooms, presence, motion, breathing rate, packet loss, noise) it emits a reproducible `CsiFrame` stream. Bit-exact given seed + generator version. |
| `Simulator` | wires a whole scene | Multi-node, three-floor scene runner through the fakes; drives the **real** core pipeline and the private control plane for integration tests. |
| `FakeSink`, `InMemoryMetricSink` | `Sink`, `MetricSink` | Capture emitted estimates and metrics for assertion. |
| `Fixture` loader | — | Parses a fixture's `manifest.json` + payloads, verifies digests, and exposes typed frames and labels. |
| `Golden` helpers | — | Canonical serialisation and hashing of a `ReferenceOutput`. |

Generator contract: a **pinned integer PRNG** with a specified seed-derivation,
elementary arithmetic only, and no unpinned transcendental functions, so a given
`(seed, generator-version)` reproduces bit-for-bit on the canonical target.

## 4. Fixtures: format and provenance

Fixtures are immutable, self-describing, and content-addressed. Layout:

```
fixtures/
  <fixture-id>/
    manifest.json
    frames.f32le        # payloads, little-endian, digest-pinned
    labels.json
    expected.json       # golden outputs for the conformance tier
```

`manifest.json` schema (`aquila.fixture/1`):

```jsonc
{
  "schema": "aquila.fixture/1",
  "id": "indoor-livingroom-presence-0001",
  "kind": "recorded",                       // synthetic | recorded | derived | external
  "capture": { "device_model": "ESP32-S3", "centre_freq_hz": 2412000000,
               "bandwidth_hz": 20000000, "subcarriers": 64, "antenna_pairs": ["0-1"] },
  "provenance": { "source": "...", "operator": "...", "captured_at": "...",
                  "calibration_id": "...", "consent": "...", "licence": "internal",
                  "redactions": ["mac", "ssid"] },
  "payloads": [ { "path": "frames.f32le",
                  "layout": "f32le interleaved [frame][rx][subcarrier](re,im)",
                  "sha256": "..." } ],
  "labels": { "path": "labels.json", "kind": "ground-truth", "source": "manual" },
  "expected": { "path": "expected.json", "digest": "..." }
}
```

Provenance classes and rules:

- **`synthetic`** — emitted by `DeterministicCsiGenerator`; records seed and
  generator version. May **never** back an accuracy claim.
- **`recorded`** — captured from real ESP32 nodes; records capture metadata and
  what was redacted. MACs, SSIDs and personal identifiers are removed or hashed
  at capture time.
- **`derived`** — produced from a parent fixture by a declared transform;
  records the parent id, parent digest, and transform name/version.
- **`external`** — from a public dataset (e.g. MM-Fi), with dataset name,
  version, citation and licence. Excluded from the shipped tree when the licence
  forbids redistribution; fetched on demand against a pinned digest instead.
  Research-only.

Rules: payloads are little-endian with no implicit byte order; every payload is
digest-pinned; fixtures are never edited in place — a change is a new id; the
`schema` field guards parsing.

## 5. The deterministic reference pipeline

The reference pipeline is a **bit-exact test asset** and the single tripwire for
drift across the whole core.

- **What it is.** One canonical configuration, `testkit::reference_pipeline()`:
  a fixed module set (preprocess → calibrate → extract features → fuse → stub
  pose/vitals models → decide), fixed parameters, a `FakeClock`, a seeded
  generator (or a fixed recorded fixture), single-threaded with a fixed float
  reduction order.
- **The asset.** Its serialised output, `ReferenceOutput`, is committed at
  `fixtures/reference/pipeline-v1/` alongside the input spec and a digest. The
  canonical form orders fields by a declared key and emits each float as its
  IEEE-754 bit pattern in hex (plus a human-readable decimal), so equality is
  unambiguous and diffs are legible.
- **Bit-exactness conditions.** Pinned Rust toolchain and target triple
  (`x86_64-unknown-linux-gnu`), no fast-math, deterministic RNG, no wall-clock,
  deterministic parallel reduction (or single-threaded where parallelism would
  be non-deterministic). Any deviation fails the test.
- **Cross-target honesty.** The bit-exact check runs only on the canonical
  target. On other targets a *weaker*, explicit quantised comparison is run and
  labelled non-authoritative — never silently substituted.
- **Changing it.** Regeneration is an explicit, reviewed act with a stated
  reason and a provenance bump; it is never a reflex to make a red test green.

## 6. Coverage and mutation targets

Coverage is a floor, not a goal; uncovered lines must be justified by name
rather than chased. Mutation testing targets the **core modules**, where a
surviving mutant means a real gap in decision logic.

| Module | Region coverage floor | Mutation (caught) | Notes |
| --- | --- | --- | --- |
| `aquila-signal` DSP | 85% | 80% | numeric kernels prioritise properties over mutation |
| `aquila-sensing` calibration | 95% | 90% | critical path |
| `aquila-sensing` fusion / decision | 90% | 85% | control-flow heavy: mutation is high-value here |
| `aquila-inference` inference glue | 85% | n/a | verified by conformance/golden outputs, not mutants |
| `aquila-testkit` | 80% | n/a | fakes are contract-tested against real adapters |
| adapters (edges) | excluded from core floor | n/a | adapter contract tests against recorded capture |

Exclusions: generated bindings (WASM/FFI glue), `main`/wiring, proc-macro
output. Mutation testing (`cargo-mutants`) runs nightly, not per-PR, until it is
stable; survivors are triaged as a real gap (add a test) or an equivalent mutant
(document and exclude with a reason).

## 7. The honest-claims contract

The documentation claims table (`docs/README.md`, the **Claims** section) and a
machine-readable registry are a **bijection**. A claim without evidence is a
defect; a registry entry with no table row is orphaned.

- **Registry.** `claims.toml` maps each `claim_id` (`CLM-<slug>`) to its claim
  text, its metric definition, its status and, when `measured`, a commit-keyed
  ledger entry.
- **Claims table.** `docs/README.md` carries one row per claim id. The verifier
  (`cargo run -p xtask -- claims-verify`) parses both the registry and the table
  and fails unless their `CLM-<slug>` sets are identical: every registry claim
  must have a table row, and no table row may be orphaned.
- **Evidence linkage (intended, not yet enforced).** Conformance and evaluation
  tests/benches are meant to declare their `claim_id` (metadata comment or
  harness config) so the verifier could also fail an orphaned test; the verifier
  does **not** parse test declarations yet. What it enforces today is the
  table↔registry bijection and the `measured`-needs-ledger rule.
- **Status vocabulary** — enforced in prose as well as data:
  - `measured` — has a ledger-recorded value from an evaluation run, pinned to a
    commit. Prose may quote this value.
  - `target` — the v1 bar is stated (e.g. torso-PCK@20 ≥ 65%, presence F1 ≥ 0.9,
    breathing MAE ≤ 2 brpm) but not yet measured or not yet met. Must not be
    presented as achieved.
  - `future` — explicitly outside v1 scope.
- **No unbacked claim.** The verifier fails a `measured` claim with no ledger
  entry, a claims-table row with no registry link, and an orphaned registry
  entry.

## 8. CI topology

| Stage | Runs | Gates merge |
| --- | --- | --- |
| `fmt`, `clippy`, rustdoc tests | every PR | yes |
| Tiers 0–3 | every PR | yes |
| Coverage floor | every PR | yes |
| Reference-pipeline bit-exactness | every PR (canonical target) | yes |
| Claims verifier | every PR | yes |
| Mutation testing | nightly | no |
| Evaluation (tier 4) | release/on demand | no |
| Simulator soak, external-fixture fetch | nightly | no |

Measurement is first-class but not yet gating: evaluation harnesses write to the
commit-keyed ledger and diff against the previous run. The ledger is the source
of truth for claim values; record and diff, no gate in v1.

## 9. TDD workflow

A change moves outward through the tiers: a failing unit test (0) defines the
interface and the behaviour; a conformance fixture (1) pins it against recorded
or golden data; a property test (2) generalises the invariant; a simulator
scenario (3) proves the wiring; an evaluation run (4) — when the change touches
accuracy — records a new ledger value and, if it backs a claim, updates the
registry. Nothing merges until 0–3 and the verifier are green.

## 10. Non-goals and prohibited patterns

- No accuracy gating in CI in v1 (record and diff only).
- No `synthetic` fixture backing an accuracy claim.
- No tests reaching past a public interface, and no test-only code paths.
- No committed agent scaffolding; `aquila-testkit` is owned product code.
- No silent cross-target substitution for the bit-exact reference check.
- No editing a fixture in place; no claim with no evidence.
