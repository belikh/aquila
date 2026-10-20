# Management dashboard — specification

Status: draft for review (v1).
Owner: dashboard author.
Scope: the Aquila management dashboard only. The sensing core, add-on
packaging, the HACS custom component, the calibration algorithm and the models
are specified elsewhere; this document references them but does not define them.

## 1. Purpose

The dashboard is the single management and monitoring surface for a
self-hosted Aquila home. It lets the operator:

- configure the physical model: **home → floors → rooms → nodes → node-pairs**;
- run the guided **global whole-home calibration** wizard;
- view **live sensing**: presence/occupancy, person count, coarse pose,
  breathing and heart rate;
- view **system health**: node liveness and link quality, time-sync and drift,
  CSI ingest rate, pipeline load, active model versions and their in-domain
  metrics;
- read the **commit-keyed measurement ledger** so every claim stays honest and
  reproducible.

Audience: the home operator, on desktop and phone, embedded in the Home
Assistant UI.

### Non-goals (v1)

- No DSP, inference, fusion, calibration mathematics, or sensing-state
  persistence. All of that lives in the core. The dashboard is a presentation
  and intent layer only.
- No SDK, no public REST/WS/MCP. The dashboard consumes the core's **private**
  control plane; it is not a public surface.
- No plugin or extension points — in-tree only.
- No direct device control. The dashboard never talks to nodes; it asks the
  core, and the core drives nodes over its own control channel.
- No browsing/search/personalisation features beyond what the flows below need.

### Thin-UI principle

The dashboard renders server truth and sends user intent. It aggregates only
for presentation, keeps no sensing history, performs no inference, and treats
the core as the single writer of all state.

## 2. Stack

Rust → WebAssembly single-page application in **Leptos** (client-side
rendering), bundled with **Trunk**, embedded in the add-on image.

### Why Leptos over Dioxus

- **Fine-grained reactivity, no virtual DOM.** Signals update only the nodes
  that changed. Live presence/pose arrive at up to ~10 Hz per room; a VDOM diff
  on every frame is wasted work and wasted WASM heap. Direct DOM updates fit
  streamed telemetry better.
- **Small, deterministic CSR bundle.** Mature `wasm-bindgen` and
  `leptos_router`; a simple, fully self-hosted Trunk pipeline with no runtime
  asset server — which suits an offline, ingress-only panel.
- **No unused platform layers.** Dioxus targets desktop/mobile/fullstack,
  none of which Aquila needs; that scope is dead weight and enlarges the audit
  surface. Leptos's scope matches ours, consistent with "no committed agent
  scaffolding".
- **Primitives map onto "thin UI".** Leptos resources/actions and strong typing
  express "fetch intent, render truth" cleanly.

Dioxus's advantages here (React-like familiarity, hot reload) are outweighed by
the streaming-render model and bundle/scope fit above.

### Shared contract

Request/response and stream types live in one crate (`aquila-types`, serde),
compiled for both native (core) and `wasm32-unknown-unknown` (dashboard), so the
API contract has a single source of truth.

## 3. Architecture and boundaries

| Crate | Role | May depend on |
| --- | --- | --- |
| `aquila-dashboard` | Leptos SPA; HTTP client (v1 polls); UI | `aquila-types`, `aquila-api` |
| `aquila-types` | API and stream types; leaf | — |
| `aquila-api` | Private HTTP+WS control plane; server | `aquila-types`, `aquila-geometry`, `aquila-sensing`, `aquila-sources`, `aquila-ha` |

Enforced DAG: `dashboard → api → types`. Workspace checks (`cargo-deny` and a
dependency-DAG check) reject any edge from `aquila-dashboard` into the sensing
core (`aquila-sensing`).

### Test seam

The UI talks to a single trait so screens can be tested headless against
recorded fixtures, matching the project's fake-adapter pattern:

```rust
/// Client seam for the core's private control plane.
trait CoreApi {
    async fn get_home(&self) -> Result<HomeTree, ApiError>;
    async fn get_state(&self) -> Result<Snapshot, ApiError>;
    async fn patch_room(&self, id: RoomId, patch: RoomPatch) -> Result<Room, ApiError>;
    // ...
}
```

`HttpCoreApi` is the real implementation (HTTP fetch + poll scheduler);
`FakeCoreApi` serves recorded fixtures. Components and flows are written against
the trait. There is no `subscribe`/`EventStream` in v1 (§5): the seam is a poll
loop, not a socket.

## 4. Serving and Home Assistant ingress

The add-on image contains two artefacts: the `aquila-addon` binary and the built
dashboard `dist/` (WASM, JS glue, CSS, bundled icon sprite). **One** HTTP server
inside the add-on serves both the static assets and the `/api/v1/*` control
plane on the same port, so the panel is same-origin and needs no CORS.

Indicative add-on manifest (`config.yaml`):

```yaml
ingress: true
ingress_port: 8099
panel_title: Aquila
panel_icon: mdi:wifi
panel_admin: false
```

- Supervisor authenticates the Home Assistant user and reverse-proxies the
  panel from the sidebar at `/api/hassio_ingress/<token>/`.
- The add-on port is **not** published. The only routes to the API are
  Supervisor ingress and the internal Supervisor network used by the HACS
  component. There is no public listener, satisfying "no public REST/WS".
- The ingress prefix is dynamic (token-based). The static server reads the
  `X-Ingress-Path` request header and injects the external prefix into
  `index.html` as `<base href>` and `window.__AQUILA_BASE__`. At boot the app
  derives a single `api_base()` from that value, falling back to the current
  origin when the header is absent (local development). **All** HTTP and WS URLs
  are built from `api_base()`; no absolute paths are hard-coded.
- In-app navigation uses **hash routing** (`#/rooms/<id>`), which is robust
  under a changing ingress token and needs no server-side rewrite or fallback.
- Embedding: the dashboard runs inside Home Assistant's ingress frame/webview.
  It reads HA theme CSS variables where they are reachable and otherwise ships a
  self-contained theme with light/dark support driven by
  `prefers-color-scheme`.

## 5. Core private API (HTTP + WS)

All paths are prefixed by `api_base()`. Bodies are JSON in `aquila-types`.
**The endpoint contract is `core-api.md` §6; this section does not restate it.**
The dashboard consumes the canonical routes there — there are no aliases and no
second table (`sister-issues-specs.md` D13). The routes it uses are:

- snapshot and structure: `GET /state`, `GET /schema`, and the home → floor →
  room → node → node-pair CRUD of `core-api.md` §6 (`/home`, `/floors`,
  `/rooms`, `/nodes`, `/node-pairs`);
- the calibration wizard: `POST /calibration`, `GET /calibration/{id}`,
  `GET /calibration/{id}/steps/{stepId}`,
  `POST /calibration/{id}/steps/{stepId}/{complete,skip}`,
  `POST /calibration/{id}/{commit,abort}`;
- system health and metrics: `GET /health`, `GET /capabilities`,
  `GET /metrics/snapshot`.

Node registration is `POST /nodes` (commission); there is no separate
`/nodes/pairing` route in the canonical set.

### WebSocket — not in v1: polling

v1 serves **no WebSocket**: `GET /api/v1/stream` returns a typed `501`
(`core-api.md` §10). The dashboard therefore **polls** the routes above (with
backoff on failure) and renders each response in place; it opens no socket and
does not reconnect-loop. The single multiplexed stream described in earlier
drafts of this document is deferred with the core's stream: when it lands it
will be the `aquila.v1` envelope of `core-api.md` §10, and only then does the
dashboard switch from polling to push.

HTTP is used for topology CRUD, the initial load, ledger reads and (in v1) live
updates; the client coalesces renders itself.


## 6. Screens and flows

### Screen map

| # | Screen | Route | Primary data | Key actions |
| --- | --- | --- | --- | --- |
| 1 | Shell / ingress frame | `#/` | — | navigate; system status pill; home selector |
| 2 | Overview | `#/overview` | `GET /state` (presence/count), `GET /health` | drill into a room; jump to health |
| 3 | Room live view | `#/rooms/{id}` | `GET /state` (presence/vitals) | select room; pause/resume live |
| 4 | Structure | `#/structure` | `GET /home` | create/rename/reorder/delete floors, rooms |
| 5 | Nodes | `#/nodes` | `GET /nodes`, `GET /health` | register, reassign, remove, inspect |
| 6 | Node pairs | `#/pairs` | `GET /node-pairs` | create/remove pairs; view coherence |
| 7 | Calibration wizard | `#/calibrate` | `GET /calibration/{id}` | start; follow steps; apply global profile |
| 8 | System health | `#/health` | `GET /health`, `GET /metrics/snapshot` | inspect node/pipeline/time-sync; view logs link |
| 9 | Measurement ledger | `#/measurements` | `GET /metrics/snapshot` | filter; view commit detail + diff |
| 10 | Settings | `#/settings` | `GET /home` | units, locale, theme, privacy, about/licence |

### Key flows

**First run / onboarding.** When no home exists, a linear wizard: create home →
add floors → add rooms → pair nodes → accept auto-suggested pairs → run
calibration. Each step writes through the API and advances only on success.
Because measurement is first-class, calibration records the baseline commit
before the dashboard reports the home as "ready".

**Node registration.** The node is flashed and joins the core's control
channel; it appears as *pending* in Nodes → the operator commissions it into a
room via `POST /nodes` (or rejects it). There is no separate pairing route in
the canonical set (`core-api.md` §6).

**Calibration wizard (global).** Preconditions are checked first (all nodes
online, time-sync healthy). Steps: empty-home baseline → per-floor/room
reference positions → activity/breathing reference → review results (per-node
corrections, residual error, resulting commit) → apply. Progress is re-read via
`GET /calibration/{id}` on the poll interval; the operator can advance, skip or
retry a step, or abort. The applied global calibration profile and its age are
shown on Overview and Health.

**Live monitoring.** Overview shows whole-home occupancy and per-room vitals
cards; Room live view drills in with a coarse top-down zone/pose view and vitals
sparklines. Everything is driven by a single poll of `GET /state` per interval,
scoped to the visible rooms.

**Change management.** Structural edits (floors/rooms/nodes/pairs) are ordinary
CRUD; after any change that invalidates calibration, the UI surfaces a clear
"recalibrate" banner rather than silently using stale corrections.

**Failure and empty states.** If a poll fails or the core is unreachable, the
dashboard keeps the last snapshot, marks it stale, and retries with backoff.
Missing rooms/nodes show explicit empty states with the next action, never blank
panels. CRUD failures are surfaced with the core's error, not swallowed.

## 7. Accessibility

Target WCAG 2.2 AA.

- Semantic HTML landmarks (`header`, `nav`, `main`, `aside`), one `h1` per
  screen, logical heading order.
- Full keyboard operability, including the topology tree and wizard; visible
  focus rings; no keyboard traps; focus moved and announced on route change.
- Live updates use a single `aria-live="polite"` region, **throttled and
  summarised** (for example "Kitchen: two people") so streamed telemetry does
  not flood screen readers. Pose/vitals updates are not announced per frame.
- Status is never conveyed by colour alone: online/offline, good/poor and
  present/absent each carry an icon and text as well as colour.
- Contrast ≥ 4.5:1 for body text and ≥ 3:1 for large text and UI components, in
  both light and dark themes.
- Charts and sparklines have a text summary and an accessible data table
  equivalent; no information exists only in a plot.
- Forms use explicit labels, inline error text tied by `aria-describedby`, and
  no placeholder-only labels.
- Respects `prefers-reduced-motion` (no gratuitous animation; live views update
  in place rather than animating) and supports text zoom to 200% without loss.
- Verified to work in the Home Assistant mobile-app webview.

## 8. Offline, no external CDN, security

- **Offline / self-hosted only.** Every asset is built into and served by the
  add-on: WASM, JS glue, CSS, fonts, icons. No CDN, no external font service, no
  web-font fetches, no analytics, no telemetry, no third-party calls at runtime.
- **Fonts:** system font stack; any bundled font is subset and shipped locally.
- **Icons:** a bundled inline-SVG sprite; no icon CDN.
- **CSP** served by the add-on's static server:
  `default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self';
  img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors
  'self'; base-uri 'self'` (`'wasm-unsafe-eval'` is required for WASM
  instantiation; `connect-src 'self'` covers same-origin HTTP and WS under
  ingress).
- **Build determinism:** pinned toolchain and lockfiles; vendored/locked
  dependency graph so a given commit reproduces the same `dist/`.
- **Trust boundary:** the panel is reachable only through authenticated
  Supervisor ingress; the dashboard sends no credentials of its own and does not
  persist secrets. The core's API stays private.
- **Privacy:** recording toggles and retention are set in Settings and enforced
  by the core, not the UI; the dashboard only reflects and requests them. The
  About section states GPLv3 and non-commercial use.

## 9. Testing and fixtures

- `FakeCoreApi` with recorded HTTP fixtures drives component and flow tests; no
  network in unit/component tests.
- Tests cover: topology CRUD rendering and validation, poll refresh and
  stale-on-failed-poll behaviour, the calibration wizard state machine
  (advance/skip/retry/abort), and accessibility-critical structure (landmarks,
  labels, live-region throttling).
- Optional WASM-level smoke tests via `wasm-bindgen-test` for the transport
  adapter. No browser automation is committed as part of v1.

## 10. Open questions

- Confirm whether HA theme variables are reachable inside the ingress frame at
  the supported HA versions, or whether the self-contained theme is always used.
- Whether node pairing should also offer a QR/serial path in v1 or only the
  text code.
- **Ledger view scope — settled.** v1 exposes the **current** commit's snapshot
  only (`GET /metrics/snapshot`, `core-api.md` §6/§13); the dashboard has no
  `GET /measurements/{commit}` route. Arbitrary-commit diffing is deferred until
  a core-api read contract for it exists.
