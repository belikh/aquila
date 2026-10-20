# Aquila delivery plan

How the v1 work is structured, and how an implementation agent consumes it.

This folder is the **wayfinder map and its tickets** for v1. It plans; the spec
and ADRs decide. Nothing here overrides `CONTEXT.md`, the spec, or an ADR.

## Numbering (one number only)

There is exactly **one ticket number, and it is the GitHub issue number**. The
map is issue **#1**; tickets are **#2–#15**. A ticket file is
`tickets/<issue-number>-<slug>.md`, its heading is `# <issue-number> — <title>`,
and its `Blocked by:` line uses the same numbers (`#n`). There is no second,
offset numbering.

- `map.md` — the wayfinder map.
- `tickets/<issue-number>-<slug>.md` — one ticket per file.
- `issues.md` — the map from issue number to file, and the live status.
- `hardware-bring-up.md` — the runbook for the day the ESP32-S3 boards arrive.
  Not a ticket: it is the operational plan plus the honest list of software
  prerequisites (P1–P6) that must close before a real node is useful.

## How an implementation agent consumes a ticket

Work one ticket per fresh session. Read outward from the ticket:

1. **Map** — `docs/plan/map.md`: the Destination and Decisions-so-far orient you
   before you touch anything.
2. **Frontier ticket** — take the first ticket (lowest issue number) that is
   open, unblocked and unassigned. A ticket is **unblocked** when every issue on
   its `Blocked by:` line is closed. Ticket `#2` (the spine) depends on `#12`
   (reconciliation), which is already closed.
3. **Spec** — the ticket's `Implements:` paths. These are normative. Read the
   owning spec file(s) in full, not a summary.
4. **ADR** — always `docs/adr/0003-interface-reconciliation.md` (the interface
   register), plus `0001` (module architecture) and `0002` (toolchain/repo), and
   `0004` (on-air CSI layout) where relevant. ADRs win over specs.
5. **Reference** — cite external claims from `docs/reference/sources.json` by
   `id`. Research under `docs/research/` is advisory and never defines scope.

If a ticket contradicts the spec or an ADR, **stop and surface it** — do not
silently follow the ticket. A contradiction is a defect in one of them.

## Ticket format

Each ticket carries, in order: a title; a `Type:` line (`research`,
`prototype`, `grilling`, or `task`); a `Blocked by:` line (comma-separated issue
numbers prefixed `#`, or `none`); a `Status:` line (`open`, `in-progress`, or
`done`); then a body with Goal, Implements (spec/ADR paths), References (source
ids), Acceptance criteria (metrics or honest-claim gates), and a Test-first first
step.

## Conventions

- **One topic per file**; lowercase, hyphenated, stable filenames, prefixed by
  the GitHub issue number.
- **Australian English.**
- **Honest claims only.** Every metric gate cites its ledger row and owner; a
  slice is not done until its `MetricSink` rows exist at the bar and a diff is
  available (`roadmap.md`).
- **Tracer bullets.** Each ticket cuts the whole pipeline end to end and is
  measured from its first commit (`roadmap.md`).
- Tickets are **published as GitHub issues**; this folder mirrors them. Never
  introduce a second numbering — reference a ticket only by its issue number.
