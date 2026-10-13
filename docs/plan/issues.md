# Plan — published issues

The v1 plan is published on GitHub at `belikh/aquila`. The in-repo files under
`docs/plan/tickets/` are the source drafts and use plan numbers (01–11); the table below maps
them to issue numbers. Blocking edges are recorded as native GitHub issue dependencies; work the
frontier (the first open, unblocked, unassigned ticket in map order).

| Plan file | Issue | Title | Blocked by |
| --- | --- | --- | --- |
| map | #1 | Aquila v1 — wayfinder map | — |
| 11-interface-reconciliation | #12 | Interface reconciliation | none (frontier) |
| 01-spine-presence-tracer-bullet | #2 | Spine tracer bullet | #12 |
| 02-vitals-tracer-bullet | #3 | Vitals tracer bullet | #2 |
| 03-global-whole-home-calibration | #4 | Global whole-home calibration | #3 |
| 04-multi-node-single-room | #5 | Multi-node single room | #4 |
| 05-floor-aggregation | #6 | Floor aggregation | #5 |
| 06-cross-floor-whole-home | #7 | Cross-floor / whole-home | #6 |
| 07-live-nodes-and-node-control | #8 | Live nodes and node control | #7 |
| 08-hacs-integration | #9 | HACS integration | #8 |
| 09-management-dashboard | #10 | Management dashboard | #8 |
| 10-packaging-and-v1-release | #11 | Packaging and v1 release | #9, #10 |

## Working the plan

1. Read the map (#1), `docs/plan/map.md`, and `CONTEXT.md`.
2. Find the frontier: open, unblocked, unassigned, lowest plan number.
3. Claim it (`gh issue edit <n> --add-assignee @me`), then read the ticket and the spec/ADR files it names.
4. Implement test-first; cite `docs/reference/sources.json` ids for any external claim.
5. Close with a comment linking the change and the ledger evidence.
