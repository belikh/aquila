# 10 — Management dashboard

- Type: task
- Blocked by: #8
- Status: open

## Goal

The add-on serves a Rust→WASM management dashboard over sidebar ingress: author
the home/floor/room/node/node-pair topology, trigger global calibration, view
live sensing and system health, and read the commit-keyed ledger. Management
only — never a sensing path.

## Implements

- `docs/spec/roadmap.md` Slice 8.
- `docs/spec/dashboard.md` §2 (Leptos/Trunk), §3 (boundaries, `CoreApi` seam),
  §4 (ingress, hash routing), §5 (private API use), §6 (screens, flows), §7
  (accessibility WCAG 2.2 AA), §8 (offline, CSP, determinism), §9 (testing).
- `docs/spec/core-api.md` (topology CRUD, calibration wizard, `/stream`, ledger).
- `docs/spec/geometry-calibration.md` §8 (wizard steps).
- `docs/spec/privacy-security.md` G12 (sandboxed dashboard, strict CSP).

## References

`leptos` (chosen stack), `dioxus` (rejected alternative); `ha-addons` (ingress).

## Acceptance criteria

- **Ingress gate:** the dashboard loads via HA sidebar ingress with no separate
  port; all URLs derive from `api_base()`; hash routing under a changing token.
- **Topology gate:** the full topology can be authored in the UI and persists
  through the host; `topology.roundtrip.digest` is stable.
- **Calibration gate:** global calibration runs from the UI and its artefact is
  visible and versioned.
- **Boundary gate:** the dashboard performs no DSP/inference and keeps no sensing
  history; ledger metrics are read-only (`dashboard.ingress.ok`).
- **Accessibility gate:** WCAG 2.2 AA — landmarks, keyboard operability,
  throttled `aria-live`, status never by colour alone, ≥ 4.5:1 contrast.
- **Offline gate:** no CDN, no third-party origin, no telemetry; strict CSP with
  `wasm-unsafe-eval`.

## Test-first first step

Write a failing component test against a `FakeCoreApi` with a recorded topology
fixture, asserting the structure screen renders floors and rooms and validates a
create-room action.
