# Plan — published issues

The v1 plan is published on GitHub at `belikh/aquila`. Files under
`docs/plan/tickets/` are named by — and mirror — the GitHub **issue number**, so
there is a **single number everywhere**: the map is #1, tickets are #2–#15.
Blocking edges are recorded as native GitHub issue dependencies.

| Issue | File | Title | Blocked by |
| --- | --- | --- | --- |
| #1 | `map.md` | Aquila v1 — wayfinder map | — |
| #2 | `02-spine-presence-tracer-bullet.md` | Spine tracer bullet | #12 |
| #3 | `03-vitals-tracer-bullet.md` | Vitals tracer bullet | #2 |
| #4 | `04-global-whole-home-calibration.md` | Global whole-home calibration | #3 |
| #5 | `05-multi-node-single-room.md` | Multi-node single room | #4 |
| #6 | `06-floor-aggregation.md` | Floor aggregation | #5 |
| #7 | `07-cross-floor-whole-home.md` | Cross-floor / whole-home | #6 |
| #8 | `08-live-nodes-and-node-control.md` | Live nodes and node control | #7 |
| #9 | `09-hacs-integration.md` | HACS integration | #8 |
| #10 | `10-management-dashboard.md` | Management dashboard | #8 |
| #11 | `11-packaging-and-v1-release.md` | Packaging and v1 release | #9, #10 |
| #12 | `12-interface-reconciliation.md` | Interface reconciliation | none |
| #13 | `13-ota-control-and-core-image-store.md` | OTA control channel and core image store | #8 |
| #14 | `14-firmware-ota-agent.md` | ESP32-S3 firmware OTA agent | #8, #13 |
| #15 | `15-ha-firmware-update-entity.md` | Home Assistant firmware update entity | #9, #13 |

## Status (verify with `gh issue list --state all`)

- Closed (done): **#2–#15** (the whole plan; note #16 was never created).
- Open: **#1** (the map itself), **#17** (run and verify the HACS integration +
  add-on on a live HA), **#18** (component should discover the core add-on
  instead of a hand-entered hostname).

#17 and #18 are post-plan operational issues found during live verification, not
frontier tickets from the map.

## Working the plan

1. Read the map (#1), `docs/plan/map.md`, and `CONTEXT.md`.
2. Frontier = the first **open, unblocked, unassigned** issue, lowest number first
   (leftmost eligible row above).
3. Claim it (`gh issue edit <n> --add-assignee @me`), then read the ticket and the
   spec/ADR files it names.
4. Implement test-first; cite `docs/reference/sources.json` ids for any external claim.
5. Close it with a comment linking the change and the ledger evidence, and let the
   next dependent issue become the frontier.
