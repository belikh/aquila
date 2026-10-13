# Aquila delivery plan

How the v1 work is structured, and how an implementation agent consumes it.

This folder is the **wayfinder map and its tickets** for v1 (see
`map.md`). It plans; the spec and ADRs decide. Nothing here overrides
`CONTEXT.md`, the spec, or an ADR.

- `map.md` — the wayfinder map: destination, scope, decisions so far, fog.
- `tickets/NN-<slug>.md` — one ticket per file, numbered from `01`.

## How an implementation agent consumes a ticket

Work one ticket per fresh session. Read outward from the ticket:

1. **Map** — `docs/plan/map.md`: the Destination and Decisions-so-far orient you
   before you touch anything.
2. **Frontier ticket** — `docs/plan/tickets/NN-<slug>.md`. A ticket is
   **unblocked** when every ticket on its `Blocked by:` line is done (its
   `Status:` is `done`). Take the first unblocked, unstarted ticket in number
   order. Ticket `01` (the spine) depends on `11` (reconciliation): take `11`
   first.
3. **Spec** — the ticket's `Implements:` paths. These are normative. Read the
   owning spec file(s) in full, not a summary.
4. **ADR** — the ADRs the ticket's area touches: always
   `docs/adr/0003-interface-reconciliation.md` (the interface register), plus
   `0001` (module architecture) and `0002` (toolchain/repo). ADRs win over specs.
5. **Reference** — cite external claims from `docs/reference/sources.json` by
   `id`. Research under `docs/research/` is advisory and never defines scope.

If the ticket contradicts the spec or an ADR, **stop and surface it** — do not
silently follow the ticket. A contradiction is a defect in one of them.

## Ticket format

Each ticket carries, in order: a title; a `Type:` line (`research`,
`prototype`, `grilling`, or `task`); a `Blocked by:` line (comma-separated
ticket numbers, or `none`); a `Status:` line (`open`, `in-progress`, or `done`);
then a body with Goal, Implements (spec/ADR paths), References (source ids),
Acceptance criteria (metrics or honest-claim gates), and a Test-first first step.

## Conventions

- **One topic per file**; lowercase, hyphenated, stable filenames.
- **Australian English.**
- **Honest claims only.** Every metric gate cites its ledger row and owner; a
  slice is not done until its `MetricSink` rows exist at the bar and a diff is
  available (`roadmap.md`).
- **Tracer bullets.** Each ticket cuts the whole pipeline end to end and is
  measured from its first commit (`roadmap.md`).
- **Do not create GitHub issues** for these tickets in this plan pass; they live
  here as local files. Publishing to the tracker is a separate, deliberate act.
