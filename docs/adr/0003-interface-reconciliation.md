# ADR-0003: Interface reconciliation — one crate graph, one port pair

- Date: 2026-10-13
- Status: Accepted
- Deciders: Aquila design session
- Related: `docs/spec/README.md` (Known divergences), `docs/adr/0001-module-architecture.md`,
  `docs/adr/0002-toolchain-and-repo.md`, `docs/spec/interface-register.md`

## Context

The spec was drafted by parallel authors. `docs/spec/README.md` records five
divergences that must be resolved before code exists: the crate graph, the node
data/control ports, `aquila-types` vs `aquila-contracts`, the source-seam name,
and the `sensing frame` definition. Each parallel spec is internally consistent
but names the same thing differently:

- ADR-0001 and `architecture.md` fix **eleven** crates and a six-layer DAG.
- `capture-and-firmware.md` adds `aquila-wire`, `aquila-session`,
  `aquila-simulator`, `aquila-capture` and names the source crate
  `aquila-source`.
- `fusion-sync.md` names `aquila-core`, `aquila-timesync`, `aquila-fusion`.
- `geometry-calibration.md` names `aquila-units`, `aquila-calibration`.
- `data-contracts.md` names `aquila-contracts` (and `aquila-csi`,
  `aquila-topology`, `aquila-measure`).
- The roadmap's provisional crate map names `aquila-presence`, `aquila-vitals`,
  `aquila-pose`, `aquila-model`, `aquila-home`, `aquila-groundtruth`,
  `aquila-sink`, `aquila-host`.
- `capture-and-firmware.md` uses UDP `9100`/`9101`; `home-assistant.md`
  publishes UDP `6969`.

Two of these are worth naming plainly. `aquila-core` is used in three different
senses (shared value types, the stateful sensing pipeline, and the whole add-on),
which is exactly the ambiguity `CONTEXT.md` exists to prevent. And the HTTP+WS
port is also inconsistent (`8765`, `8099`, `8080`) even though the README list
does not call it out.

The design principles are unchanged: deep modules, small interfaces, real seams
only (two adapters), an enforced dependency DAG, TDD on fixtures, and honest,
reproducible measurement. This ADR picks one canonical set; from here, any
divergence from `docs/spec/interface-register.md` is a defect.

## Decision

### 1. One canonical crate graph

Adopt the ADR-0001 six-layer graph, refined with the three ingestion crates the
capture path genuinely needs and a dev-only capture binary. Names are singular
and settled:

| Layer | Level | Crate | Absorbs |
| --- | --- | --- | --- |
| L0 vocabulary | 0 | `aquila-types` | `aquila-contracts`, `aquila-units`, `aquila-core` (types sense) |
| L1 pure transforms | 1 | `aquila-wire` | — (new, leaf codec) |
| L1 | 1 | `aquila-signal` | — |
| L1 | 1 | `aquila-geometry` | `aquila-topology`, `aquila-home` |
| L1 | 1 | `aquila-metrics` | `aquila-measure` |
| L2 ingestion/models | 2 | `aquila-session` | — (new, `.aqs` container) |
| L2 | 2 | `aquila-inference` | `aquila-model`, `aquila-pose`, `aquila-vitals`, presence/count heads |
| L2 | 3 | `aquila-sources` | `aquila-source`, `aquila-csi` (ingest), node control |
| L2 | 4 | `aquila-simulator` | — (new, deterministic synthesis) |
| L3 core | 5 | `aquila-sensing` | `aquila-clock`/`aquila-timesync`, `aquila-fusion`, `aquila-calibration`, `aquila-presence`, `aquila-sink` (seam), `aquila-core` (pipeline sense) |
| L4 edges | 6 | `aquila-evaluation` | `aquila-groundtruth` |
| L4 | 6 | `aquila-ha` | — |
| L4 | 7 | `aquila-api` | `aquila-host` (transport) |
| L5 end adapter | 8 | `aquila-dashboard` | — |

Composition root and tooling (unlayered, exempt from the level rule):
`aquila-addon` (the composition-root binary), `aquila-capture` (dev-only
capture/synth/verify/provision binary), `aquila-testkit` (dev-only fakes and
fixtures), and `xtask` (repo tooling).

The authoritative dependency edges are in `docs/spec/interface-register.md` and
mirrored into `tools/dag.toml`; the enforced check reads that data.

### 2. `aquila-types` wins over `aquila-contracts`

The L0 vocabulary crate is **`aquila-types`**. ADR-0001, `architecture.md`, the
enforced `tools/dag.toml`, and `dashboard.md`'s DAG all already use it; choosing
`aquila-contracts` would require revising the ratified architecture decision and
the allow-list for no behavioural gain. "Contract" stays a property of the types
version (`CONTRACT_VERSION`), not a second crate. `data-contracts.md` §0.2's
`aquila-contracts` references are superseded by this ADR and its own §0.2 text is
to be corrected to `aquila-types` when next touched. The canonical type *shapes*
remain `data-contracts.md`.

### 3. Source crate is `aquila-sources`; the seam is `CsiSource`

The crate is **`aquila-sources`** (plural) and the trait is **`CsiSource`**. The
`aquila-source` spelling in the roadmap and `capture-and-firmware.md` is
superseded. `NodeControl` stays the outbound half of the same seam, not a
seventh seam (ADR-0001 §3). Node identity on the wire stays the factory MAC
(`MacAddr`); the dense `NodeId(u16)` in `data-contracts.md` is the canonical
*contract* identity and `aquila-wire` maps between them at the seam.

### 4. `aquila-wire` is a leaf codec, not a seam

Keep **`aquila-wire`** as a pure, dependency-light L1 crate that owns the one
on-air layout for both the CSI data plane and the node control plane, and that
generates `node_wire.h` for the firmware (one layout, two languages:
`capture-and-firmware.md` §9.4). The *seam* (`CsiSource`/`NodeControl`) stays in
`aquila-sources`; `aquila-wire` is the shared codec beneath it so
`aquila-session`, `aquila-simulator` and the C header generator need not depend
on the seam crate. This refines ADR-0001 §3, it does not contradict it.

### 5. `Clock` is a seam, not a crate; its trait lives at L0

There is **no `aquila-clock`, no `aquila-timesync` crate**. `Clock`,
`SystemClock`, `ReplayClock` and the `ClockSynchroniser` drift estimator live in
`aquila-sensing`. The one-method `Clock` trait and the `CoreTime`/`DeviceTime`
newtypes are declared in **`aquila-types`** so that `aquila-sources` and
`aquila-simulator` (which inject `Arc<dyn Clock>` for deterministic replay) can
depend on them without an upward edge. This is a necessary refinement of
ADR-0001 §Decision 2; the rule "no clock crate; only the core owns sync policy"
holds.

### 6. Fusion and calibration are internal modules of `aquila-sensing`

**No `aquila-fusion`, no `aquila-calibration` crate.** Time sync and per-node
drift, whole-home calibration, sliding windows and jitter, feature-level fusion,
and room→floor→cross-floor escalation are internal modules (`time`,
`calibration`, `window`, `fusion`, `escalation`) with internal seams, tested
directly, exactly as ADR-0001 §Decision 1 and §Alternatives 2 decided. Promotion
triggers remain in `architecture.md` §11.

### 7. One UDP port pair: `9100`/`9101`

Adopt **UDP `9100`** for the CSI data plane (node → core, one CSI frame per
datagram) and **UDP `9101`** for the node control plane (core ↔ node
request/response). `home-assistant.md`'s `6969` is superseded; the add-on
publishes `9100/udp` (and `9101/udp` where the control plane needs an inbound
listener). The pair is adjacent, already documented as a pair, and keeps data
and control unambiguous.

### 8. One HTTP+WS/ingress port: `8099`

Beyond the README's list, this ADR also settles the private control plane:
**TCP `8099`** is the canonical add-on HTTP + WebSocket + ingress port (the
value `home-assistant.md` and the HACS component's default core URL already use).
`core-api.md`'s `8765` and `dashboard.md`'s illustrative `8080` are superseded;
if the core still uses an internal loopback listener, it is an implementation
detail behind `8099`, not a second published port.

### 9. `sensing frame` is confirmed, not conflicting

Divergence 5 is a non-conflict. A **`CsiFrame`** is the per-packet on-wire unit
(`data-contracts.md` §2, produced by the firmware). A **`CsiEnvelope`** wraps it
with host arrival. A **`SensingFrame`** is `CONTEXT.md`'s atomic unit of
analysis: the CSI frames of one node or node-pair time-aligned onto a
`ClockGrid` over a fixed span (`data-contracts.md` §2.1). The roadmap's per-slice
use of "frame" means the wire unit; nothing is redefined.

### 10. Contract and wire versions

`CONTRACT_VERSION = 1.0`, `WIRE_MAJOR`/`WIRE_MINOR = 1.0` per
`data-contracts.md`. Both move together; a contract is append-only within a
major version.

## Alternatives considered

1. **Adopt `aquila-contracts` and rename `aquila-types`.** Rejected: it churns
   the ratified ADR-0001 decision, the `dashboard.md` DAG, and `tools/dag.toml`
   for a naming preference; ADRs win over specs, and the version field already
   carries the "contract" meaning.
2. **Keep `aquila-source` (singular) and rename `aquila-sources`.** Rejected:
   the plural crate is already in ADR-0001 and the enforced allow-list.
3. **Pay the `9100`/`9101` pair down to the single `6969`.** Rejected: one port
   cannot express the data/control plane distinction that
   `capture-and-firmware.md` depends on; the add-on's `6969` was a lone ingest
   port, not a protocol decision.
4. **Create `aquila-clock`, `aquila-fusion`, `aquila-calibration` as crates.**
   Rejected by ADR-0001 §Alternatives 2: each has a single real consumer
   (`aquila-sensing`) and would invent shallow interfaces with no second
   adapter. Revisit only if a promotion trigger in `architecture.md` §11 fires.
5. **Fold `aquila-wire`, `aquila-session`, `aquila-simulator` into
   `aquila-sources`.** Rejected: the codec, the container and the deterministic
   simulator each have independent consumers (the C header, `aquila-capture`,
   the integration harness) and belong below the seam; folding them would widen
   `aquila-sources` into a shallow grab-bag.
6. **Keep a `Clock` trait in `aquila-sensing` and let sources receive raw
   timestamps.** Rejected: `capture-and-firmware.md` §3.2 injects `Arc<dyn
   Clock>` into every source adapter for byte-identical replay; the trait must
   be reachable from L2 without an upward edge.

## Consequences

Positive:

- One crate list, one DAG, one port pair. `docs/spec/interface-register.md` is
  the register; `tools/dag.toml` is the enforced data; any other spelling is a
  defect.
- `aquila-types` remains the shared alphabet, `aquila-sensing` the single
  stateful core, and the six seams each keep exactly two adapters.
- The node wire layout has one definition (`aquila-wire`) that both the firmware
  header and the core consume, so the two languages cannot silently drift.
- The port pair makes the data plane and control plane separately observable in
  the ledger and the dashboard.

Negative / accepted costs:

- Three specs must be corrected when next edited: `data-contracts.md` §0.2
  (`aquila-contracts` → `aquila-types`), `capture-and-firmware.md` and the
  roadmap (`aquila-source` → `aquila-sources`, `6969` → `9100`/`9101`),
  `fusion-sync.md`/`geometry-calibration.md`/`signal.md`/`dashboard.md`
  (`aquila-core`/`aquila-timesync`/`aquila-fusion`/`aquila-calibration`/
  `aquila-units` → the canonical set). This ADR is the tie-breaker until then.
- The integer levels shift relative to `repository-tooling.md` §3.1 because the
  new ingestion crates are inserted; `tools/dag.toml` is regenerated from the
  register.
- `aquila-types` now carries two port *traits* (the `Clock` trait); the "no
  behaviour" rule is preserved by keeping them interface-only, with every
  implementation elsewhere.
- `home-assistant.md` must republish `9100`/`9101` instead of `6969`, and
  `core-api.md`/`dashboard.md` must align on `8099`.

## Follow-ups

- Implement `#11 interface-reconciliation` (plan ticket): scaffold the fourteen
  product crates and four tooling crates, land `tools/dag.toml` from the
  register, and pin the toolchain per ADR-0002.
- Correct the superseded names in the sibling specs when next touched
  (`data-contracts.md` §0.2 first, then `capture-and-firmware.md`,
  `fusion-sync.md`, `geometry-calibration.md`, `signal.md`, `dashboard.md`,
  `home-assistant.md`, `core-api.md`, roadmap).
- Keep `docs/spec/interface-register.md` and `tools/dag.toml` in lockstep; a new
  edge or crate is a reviewed one-file change against this ADR.
