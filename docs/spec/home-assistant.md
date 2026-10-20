# Home Assistant integration — spec

Status: settled for v1. Prose in Australian English. This document specifies
how Aquila is packaged into and surfaced by Home Assistant (HA); it does not
re-specify the sensing core, calibration or measurement work (see the core
specs and the measurement ledger).

Role: Home Assistant integration author.

## 1. Scope

Two deliverables ship to a HA user:

1. **Aquila Core** — a HA add-on (App) that runs the Rust core natively and
   serves the management dashboard (Rust → WASM) over sidebar ingress.
2. **Aquila** — a HACS custom component (Python) that talks to the add-on over
   the core's **private HTTP + WS API** and projects the sensing results into HA
   entities, a config flow and a sidebar panel.

v1 exposes no public REST/WS/MCP and no plugin surface: the HTTP + WS API in §5
is private to the component on the HA local network, and the add-on is the only
thing that does DSP, inference, fusion, calibration and measurement.

## 2. Topology and trust boundaries

```
 ESP32-S3 nodes (dumb sensors)
   │  raw CSI, UDP unicast  ─────────────────────────────┐
   ▼                                                     │
 ┌─────────────────────────── HA OS / Supervised host ───┼──────────────┐
 │                                                        │              │
 │   ┌──────────────────────────┐   core→node control     │              │
 │   │ Aquila Core add-on       │─────────────────────────┘              │
 │   │ (Rust core + WASM dash)  │                                        │
  │   │  :9100/udp  CSI ingest   │                                        │
 │   │  :8099      HTTP+WS+ingress                                        │
 │   └────────────┬─────────────┘                                        │
 │                │ hassio Docker network (not LAN-published)            │
 │   ┌────────────▼─────────────┐                                        │
 │   │ HA Core                  │                                        │
 │   │  custom_components/aquila│  ← HACS component, DataUpdateCoordinator│
 │   └──────────────────────────┘                                        │
 └───────────────────────────────────────────────────────────────────────┘
```

- The add-on runs with **bridge networking** and publishes exactly two host
  ports: UDP `9100` for CSI ingest and UDP `9101` for the node control channel
  (ADR-0003 §7). The HTTP+WS API on 8099 is **not** published
  to the host LAN; it is reachable only from HA Core / other add-ons over the
  `hassio` Docker network. That is what makes it "private".
- CSI is **unicast** UDP: nodes are provisioned with the core's host address and
  port, so no broadcast/multicast crosses the Docker NAT boundary. If broadcast
  discovery is ever added, `host_network: true` becomes required (see §10).
- The core→node control channel is outbound-only from the add-on and needs no
  inbound port.
- HA Core reaches the add-on over the `hassio` Docker network. Supervisor
  assigns the add-on hostname `<repository-hash>-<slug-with-hyphens>` (for
  example `c5ea7a93-aquila-core`), so the deployment URL is
  `http://<repository-hash>-aquila-core:8099`. The component's `DEFAULT_URL`
  constant is still the literal `http://aquila-core:8099`, which does not resolve
  for a repository-installed add-on (it would only work if the slug or alias were
  exactly `aquila-core`); automatic discovery is tracked as issue #18.

## 3. Packaging

### 3.1 Add-on (App): `aquila_core`

Standalone `config.yaml` (current Supervisor schema; `config.json` is legacy and
not used):

```yaml
name: Aquila Core
version: 0.1.0
slug: aquila_core
description: WiFi CSI sensing core for Aquila — DSP, inference, fusion, calibration
url: https://github.com/belikh/aquila
arch:
  - amd64
  - aarch64
image: ghcr.io/belikh/aquila-core-{arch}
startup: application
boot: auto
init: false
ingress: true
ingress_port: 8099
panel_icon: mdi:radar
panel_title: Aquila
panel_admin: true
ports:
  9100/udp: 9100
  9101/udp: 9101
ports_description:
  9100/udp: Raw CSI ingest from nodes (UDP unicast)
  9101/udp: Node control channel (pairing, config, firmware OTA)
options:
  log_level: info
  calibration_profile: default
  retention_days: 0   # 0 = retain indefinitely (G9); a positive value opts in
schema:
  log_level: list(trace|debug|info|notice|warning|error|fatal)
  calibration_profile: str
  retention_days: int(0,3650)
```

`retention_days` defaults to `0`, meaning **retain indefinitely**: no retention
window is imposed (G9). A positive value is an explicit, user-chosen opt-in to
automatic expiry, which the core may then run; the shipped default never deletes
data on its own.

- **Supported arches:** `amd64` (HA OS / Supervised on x86-64) and `aarch64`
  (HA OS on Raspberry Pi 4/5 64-bit, HA Yellow/Green CM4). `armv7`/`armhf`
  (32-bit) are **unsupported**: the ONNX runtime and multi-node CSI buffers are
  memory-hungry and 32-bit address space is the binding constraint. Documented
  honestly rather than built and untested.
- **Images:** prebuilt multi-arch images published to GHCR per arch
  (`...-amd64`, `...-aarch64`), built with `docker buildx` plus QEMU for the
  non-native arch. Local build via `build.yaml` is a fallback, not the default,
  because the Rust + ONNX toolchain is heavy.
- **Persistence:** all mutable state lives under `/data` (add-on persistent
  volume): the commit-keyed measurement ledger, the global calibration profile
  store, the per-node drift state and the downloaded model bundles. `/data` is
  included in HA snapshots.
- **Ingress:** `ingress: true` + `ingress_port: 8099` makes HA proxy the
  dashboard under `/api/hassio_ingress/<session-token>/`. The add-on's single
  HTTP server on 8099 serves **both** the ingress dashboard and the private
  `/api/v1/...` + `/api/v1/stream` endpoints. HA injects the `X-Ingress-Path`
  request header; the dashboard reads it and injects a matching `<base href>` so
  the WASM bundle and API calls resolve under any ingress path. Assets are
  served base-path-agnostically with relative URLs, and the WASM MIME type must
  be `application/wasm` or the loader fails behind ingress.
- **Dashboard:** compiled from the Rust dashboard crate to
  `aquila_dashboard_bg.wasm` + JS glue, bundled into the image (no `map:` needed).
  Updating the dashboard therefore happens through an **add-on** update, entirely
  independent of HACS. The panel is `panel_admin: true`: this is a management
  dashboard, so end users see sensing through HA entities (§6), not through this
  panel.

### 3.2 HACS custom component: `aquila`

Installed by HACS as an `integration`; runs inside the HA Core container.

```
custom_components/aquila/
├── __init__.py          # async_setup_entry / async_unload_entry, services
├── manifest.json
├── const.py             # DOMAIN, defaults, metric vocabulary
├── api.py               # aiohttp REST client (private HTTP API)
├── stream.py            # HTTP polling transport (v1 has no WS; see §5.2)
├── coordinator.py       # AquilaCoordinator(DataUpdateCoordinator)
├── config_flow.py       # config + options + reauth
├── binary_sensor.py     # presence, occupancy, connectivity, self-test
├── sensor.py            # person count, breathing, heart rate, health
├── update.py            # per-node firmware update entity (see firmware-ota.md)
├── diagnostics.py       # config-entry diagnostics (redacted)
├── strings.json
└── translations/en.json
```
> The branded `brand/icon.png` is **not** shipped yet (`brand/` contains only a
> README); it is deferred with the component panel (§8).

`manifest.json`:

```json
{
  "domain": "aquila",
  "name": "Aquila",
  "codeowners": ["@belikh"],
  "config_flow": true,
  "documentation": "https://github.com/belikh/aquila",
  "issue_tracker": "https://github.com/belikh/aquila/issues",
  "integration_type": "hub",
  "iot_class": "local_polling",
  "version": "0.1.0",
  "dependencies": ["frontend", "http"],
  "requirements": [],
  "loggers": ["custom_components.aquila"]
}
```

- `version` is mandatory for custom integrations and is the field HACS compares
  for updates.
- `requirements` is empty: the WS and REST clients use `aiohttp`, which HA
  already ships; no third-party dependency is pulled into HA Core. This is a
  deliberate constraint — a hub integration must not drag in its own HTTP stack.
- `iot_class: local_polling`: v1 has no WebSocket stream (`core-api.md` §10), so
  the coordinator polls `GET /api/v1/state` (and the node/firmware resources) on
  its interval. This is the primary source in v1; when the stream lands it
  becomes push with the poll demoted to reconciliation.
- `integration_type: hub`: one config entry represents one Aquila core
  instance, to which many node/zone devices belong.

### 3.3 Monorepo layout and repository strategy

Aquila is a monorepo (`/home/matt/projects/aquila`). The HA-facing pieces live
under a single top-level directory so the add-on store and HACS can both point
at `belikh/aquila`:

```
/home/matt/projects/aquila/
├── repository.yaml              # marks the repo as an add-on (App) repository
├── hacs.json                    # marks the repo as a HACS repository
├── aquila_core/                 # add-on: config.yaml, Dockerfile, build.yaml
├── custom_components/aquila/    # HACS integration (see §3.2)
├── crates/                      # aquila-<english-descriptor> Rust crates
└── docs/
```

- **Add-on store:** users add `https://github.com/belikh/aquila` as a custom
  add-on repository; `repository.yaml` at the root plus the `aquila_core/`
  folder is the standard layout.
- **HACS:** `hacs.json` at the root is exactly
  `{"name": "Aquila", "render_readme": true}` — no `zip_release`, no `filename`.
  The repository already has the HACS-required layout, with the integration under
  `custom_components/aquila/`, so HACS installs directly from the release tree
  (or the branch). Each release *also* attaches `aquila.zip` (containing
  `custom_components/aquila/`) as a release asset, but that is for manual
  installs — HACS does not use it. (`zip_release` was tried and dropped because
  the release zip layout was wrong for HACS.)
- If a single repo proves ambiguous for either store, the fallback is a separate
  `belikh/aquila-addon` repository; that is a packaging detail, not a
  behavioural one. Flagged in §10.

## 4. Naming convention

| Thing | Value | Rule |
| --- | --- | --- |
| Add-on slug | `aquila_core` | lowercase, `_` separators; full store slug `<repo>_aquila_core`; network hostname `<repository-hash>-aquila-core` (Supervisor-assigned) |
| Component domain | `aquila` | drives `custom_components/aquila`, entity service namespace, `DOMAIN` |
| Add-on TCP port | `8099` | HTTP + WS + ingress (not host-published) |
| Add-on UDP ports | `9100`/`9101` | CSI ingest + node control (host-published; ADR-0003 §7) |
| Entity unique_id | `aquila:{instance_id}:{zone_id}:{metric}` | `instance_id` from core `/api/v1/health`; `zone_id` stable slug |
| Node device id | `("aquila", "{instance_id}:node:{node_mac}")` | one HA device per node |
| Zone device id | `("aquila", "{instance_id}:zone:{zone_id}")` | one HA device per room/floor |

- Component `DOMAIN = "aquila"`. The add-on slug is `aquila_core` (the core,
  not the whole product); there is exactly one add-on in v1. This slug, the
  Supervisor-assigned `<repository-hash>-aquila-core` hostname and the
  `ghcr.io/belikh/aquila-core-*` image
  are Home Assistant deployment identities, **not** Rust crate names; the
  composition-root crate is `aquila-addon` per ADR-0003.
- `instance_id` is a random UUID generated once by the core and persisted in
  `/data`; unique_ids survive reinstall and are stable across HA restarts.
- Zone `zone_id` is the slug of the zone name (e.g. `living_room`); the
  home → floor → room tree from the domain model maps to HA device areas and to
  the `aquila_<zone>` entity-name prefix.
- Entity object ids follow HA's default derivation:
  `binary_sensor.aquila_<zone>_presence`, `sensor.aquila_<zone>_breathing_rate`,
  and so on.

## 5. Component ↔ core contract (and why not MQTT)

### 5.1 Why MQTT is not used

- **No broker, no fourth process.** MQTT would add a broker as an installation
  prerequisite and a whole config surface (broker host, credentials, topics,
  discovery prefix). The nodes already stream raw CSI over UDP to the core, and
  the core owns the entire pipeline; there is nothing for a broker to mediate.
- **The contract is typed and versioned, not a topic tree.** The component needs
  a structured snapshot: zone tree, per-node drift/calibration state, model
  bundle versions, ledger/measurement status. Flattening that into retained
  topics loses types and makes schema evolution dangerous. A versioned JSON
  snapshot (plus, when the stream lands, a sequenced event stream) is safer and
  easier to migrate.
- **MQTT discovery would fight the entity model.** Aquila entities carry
  zone-scoped unique_ids, config subentries and diagnostics; building them from
  MQTT discovery payloads is fragile compared with a direct, typed snapshot.
- **Local push is sufficient.** The core and component are on the same host and
  the same Docker network. The cost MQTT avoids (a shared broker between
  unreliable remote clients) does not apply. The HA-recommended alternative for a
  same-host hub is a direct local API; v1 polls it and adds a WS push when the
  stream lands (`core-api.md` §10).
- Non-goal reasoning: MQTT would also be the first step toward exposing Aquila
  state to arbitrary external consumers, which v1 explicitly forbids (no public
  API, in-tree only).

MQTT is not forbidden forever; it is simply the wrong seam for v1. If a future
multi-host topology appears, MQTT or the private API can be reconsidered behind
the same component.

### 5.2 HTTP API (the actual contract)

Private, local, JSON, versioned. **The endpoint contract is `core-api.md` §6**;
this section names only the read-only subset the component consumes. There are
no aliases and no second route table (`sister-issues-specs.md` D13). The core
advertises its contract version; the component refuses or degrades on mismatch
(§9).

Polls (all `GET`), base `http://<repository-hash>-aquila-core:8099` (the
Supervisor-assigned add-on hostname; see §2):

| Path | Purpose |
| --- | --- |
| `/api/v1/health` | core health. The schema is `core-api.md` §4.1 (authoritative). The component reads: `instance_id`, the version as `api` (an object, `{major, minor}` — there is no separate `api_version`), `min_component_version`, `core_connected`, `self_test_ok`, node counts and calibration state |
| `/api/v1/schema` | zone tree (home → floor → room), metric vocabulary, units |
| `/api/v1/state` | full current snapshot: every zone's presence/vitals + system health (coordinator poll + initial load) |
| `/api/v1/nodes` | per-node identity, MAC, floor/room, online state, firmware version |
| `/api/v1/firmware` | firmware image store manifest (versions, sha256, target chip) — `firmware-ota.md` §7 |
| `/api/v1/nodes/{node}/firmware` | installed version, boot partition, pending state, last OTA result |
| `/api/v1/nodes/{node}/firmware/install` (`POST`) | begin a firmware OTA for a node (typed `503 ota_unavailable` until OTA lands) |
| `/api/v1/nodes/{node}/firmware/job` | OTA job progress/state |
| `/api/v1/nodes/{node}/firmware/abort` (`POST`) | abort an in-progress OTA |
| `/api/v1/metrics/snapshot` | commit-keyed metric snapshot (`core-api.md` §13) |

#### Transport: polling, not WebSocket (v1)

v1 serves **no WebSocket**: `GET /api/v1/stream` returns a typed `501`
(`core-api.md` §10). The component therefore polls the resources above on its
coordinator interval (default 60 s) and MUST NOT open a socket or
reconnect-loop. Each `/api/v1/state` response is the same zone/health snapshot
the stream would have pushed; a failed poll marks the core unreachable and
entities unavailable until the next successful poll (`manifest.json` is
`iot_class: local_polling`).

- Poll responses are plain resources with no `seq`; staleness is judged from the
  component's own successful-poll time.
- A failed poll backs off (base 1 s, factor 2, cap 30 s, with jitter) and flips
  `core_connected`; there is no gap/resync concept because there is no sequence.
- **Auth (the private-network/ingress trust boundary).** The core authenticates
  every request by exactly one of three modes (`core-api.md` §3): a **trusted
  private peer** (loopback, RFC 1918, link-local or IPv6 ULA), the add-on
  **ingress identity header**, or a **local bearer token**. The component runs in
  the HA core container and reaches `http://<repository-hash>-aquila-core:8099`
  over the `hassio` Docker network, so its peer address is private and the
  default needs no token;
  a bearer token (`Authorization: Bearer …`) may be configured for other
  placements. A token in a **query string is always rejected** (`401`); the
  component never puts a credential in the URL. The private network is trusted
  for *placement*, not encrypted, and the add-on publishes no host or LAN port —
  so the trust boundary is the unpublished port plus the private network, not
  the loopback interface alone.

## 6. Entity model

Data shape in the coordinator snapshot:

```
{
  "zones": {
    "<zone_id>": {
      "kind": "home" | "floor" | "room",
      "name": "...",
      "parent": "<zone_id>" | null,
      "presence": bool,
      "presence_confidence": float,
      "person_count": int,
      "breathing_rate": float | null,
      "heart_rate": float | null
    }, ...
  },
  "health": {
    "core_connected": bool,
    "nodes_online": int,
    "nodes_total": int,
    "calibration_state": "uncalibrated|calibrating|calibrated|degraded",
    "clock_drift_ms": float,
    "last_packet_ts": "ISO-8601",
    "self_test_ok": bool
  }
}
```

### 6.1 Entity list

| Entity (object id) | Platform | Scope | Unit | device_class / state_class | Category |
| --- | --- | --- | --- | --- | --- |
| `binary_sensor.aquila_occupancy` | binary_sensor | home | — | `occupancy` | primary |
| `binary_sensor.aquila_<zone>_presence` | binary_sensor | room, floor | — | `presence` | primary |
| `sensor.aquila_person_count` | sensor | home | people | — (icon `account-multiple`) | primary |
| `sensor.aquila_<zone>_breathing_rate` | sensor | room, home | brpm | `measurement` | primary |
| `sensor.aquila_<zone>_heart_rate` | sensor | room, home | bpm | `measurement` | primary |
| `binary_sensor.aquila_core_connected` | binary_sensor | system | — | `connectivity` | diagnostic |
| `sensor.aquila_nodes_online` | sensor | system | nodes | — | diagnostic |
| `sensor.aquila_nodes_total` | sensor | system | nodes | — | diagnostic |
| `sensor.aquila_calibration_state` | sensor | system | — | `enum` | diagnostic |
| `sensor.aquila_clock_drift` | sensor | system | ms | `measurement` | diagnostic |
| `sensor.aquila_last_csi_packet` | sensor | system | — | `timestamp` | diagnostic |
| `binary_sensor.aquila_self_test` | binary_sensor | system | — | `problem` | diagnostic |
| `update.aquila_<node>_firmware` | update | node | — | `firmware` | diagnostic |

Rules:

- **Occupancy** is the home-level "is anyone home" aggregate; **presence** is
  per zone (rooms and floors). They are distinct entities so automations can use
  either.
- **Person count** is home-level in v1 (integer). Per-floor count is deferred to
  a later release when cross-floor fusion is trustworthy.
- **Vitals** are emitted per **room** and aggregated to **home**; a `null`
  reading maps to HA `unavailable`/`unknown` rather than a fabricated zero.
  Reference v1 bars (breathing MAE ≤ 2 brpm vs reference) live in the core
  spec; HA only projects the values.
- Presence confidence is carried in the snapshot for the dashboard but is not a
  separate entity in v1 (keeps the entity count and the recorder footprint
  bounded). Reconsider on demand.
- Every entity has `entity_category: diagnostic` where marked, so primary
  dashboards stay clean.
- Entities are named with `_attr_has_entity_name = True` and a
  `_attr_translation_key` looked up from `strings.json`, so names are
  translated and the device name supplies the prefix
  (`Aquila Living Room > Presence`).
- `diagnostics.py` dumps the coordinator snapshot and config entry with node MACs
  and any token redacted.

### 6.2 Coordinator and polling

```python
class AquilaCoordinator(DataUpdateCoordinator[dict]):
    """Caches the latest core snapshot; the HTTP poll is the v1 source."""

    def __init__(self, hass, client, update_interval):
        super().__init__(hass, _LOGGER, name=DOMAIN, update_interval=update_interval)
        self._client = client

    async def _async_update_data(self) -> dict:
        # primary source and health probe; also the initial load
        return await collect_snapshot(self._client)
```

- `_async_update_data` performs `GET /api/v1/state` and, best-effort, `GET
  /api/v1/health` (the **health probe**), `GET /api/v1/nodes` and `GET
  /api/v1/firmware`; the interval defaults to 60 s and is configurable. This is
  both the initial load and the ongoing source in v1. The probe's counters
  (`core_connected`, `nodes_online`, `nodes_total`, `calibration_state`,
  `clock_drift_ms`, `last_packet_ts`, `self_test_ok`) populate the snapshot's
  `health` block (§6); a failed probe is best-effort and leaves the `/state`
  health block in place.
- There is no `AquilaStreamClient` in v1 (no WebSocket). A failed poll flips
  `coordinator.last_update_success`, which marks entities unavailable and writes
  `core_connected = false`; the next successful poll restores both.
- Entities are `CoordinatorEntity` subclasses with `_attr_should_poll = False`;
  they read from `coordinator.data` and never hold their own transport.
- On setup failure to reach the core, the config entry raises
  `ConfigEntryNotReady`; on repeated poll failure `UpdateFailed` drives HA's
  entity-availability handling.

## 7. Config flow

`config_flow.py` implements `ConfigFlow` (domain `aquila`):

- `async_step_user`: one field, **Core URL**, defaulted to
  `http://aquila-core:8099`. That default does not resolve for a
  repository-installed add-on, where Supervisor assigns
  `<repository-hash>-aquila-core`; automatic discovery is tracked as issue #18.
  Validates by calling
  `GET /api/v1/health`; on success stores `instance_id` as the entry unique_id
  and calls `async_create_entry(data={"url": ..., "token": None})`. Duplicate
  `instance_id` aborts with `already_configured`.
- `async_step_reauth`: triggered when the API returns 401 (only relevant if a
  token is enabled); replaces the token and reloads the entry.
- `OptionsFlow`: `update_interval` (10–600 s), `expose_floors` (bool, default
  true), `expose_room_vitals` (bool, default true). Options changes trigger a
  reload.
- **Discovery (deferred):** the add-on may later advertise `_aquila._tcp` via
  Zeroconf for zero-config setup; v1 uses the writable default URL. Noted in §10.
- `async_migrate_entry` handles config-entry schema bumps; the core owns its own
  `/data` schema migrations on start.

## 8. Sidebar panel registration

**v1 has one panel: the add-on ingress panel.** Because the add-on sets
`ingress: true` with `panel_icon`/`panel_title`, HA automatically adds a
**management** sidebar entry (admin-only via `panel_admin: true`) pointing at the
ingress dashboard. This works even before the HACS component is installed, and it
is the single source of truth for the management UI.

A second, integration-owned `panel_custom` (framing core status and deep-linking
to the ingress dashboard) was specified but is **deferred**: it depends on an
`aquila-panel.js` web component that is not shipped (`sister-issues-firmware-ha.md`
F30, and the dashboard work is ticket #10). Registering a `panel_custom` whose
asset is absent only produces a silently failing panel, so the component does not
register one in v1. When the asset lands, the component will register it once per
domain and remove it when the last entry unloads, guarded by `hass.data[DOMAIN]`
ref-counting. Until then, the entry point to management is the add-on ingress
panel above.

## 9. Install and upgrade flow

**Install (fresh host):**

1. Add the add-on repository `https://github.com/belikh/aquila`;
   install **Aquila Core**; set options (`log_level`, `calibration_profile`; leave
   `retention_days` at its `0` default to retain everything, G9); start it. On
   first start the core generates `instance_id`,
   initialises `/data` (ledger, calibration store, drift state, model cache),
   binds UDP `9100`/`9101` and serves ingress on `8099`.
2. Provision nodes with the core's host address and UDP port (unicast).
3. In HACS, add `https://github.com/belikh/aquila` as a custom repository
   (integration); download **Aquila**; restart HA.
4. Settings → Devices & Services → Add Integration → **Aquila**. The URL
   defaults to `http://aquila-core:8099` — which does not resolve for a
   repository-installed add-on; use the Supervisor-assigned
   `http://<repository-hash>-aquila-core:8099` instead (issue #18). The flow
   validates `/api/v1/health` and creates the entry; the entities appear. The
   dashboard is on the add-on page's **Open Web UI**; enable **Show in sidebar**
   on the add-on page to add it to the sidebar.

**Upgrade:**

- **Add-on:** bump `version:` in `aquila_core/config.yaml`; HA shows an update;
  the user clicks Update. `/data` persists; the core runs its own schema
  migrations on start. The dashboard (WASM) updates as part of this step.
- **Component:** bump `version` in `manifest.json`; the release publishes the
  updated component tree (and rebuilds the manual-install `aquila.zip`); HACS
  shows an update; the user updates and restarts. `async_migrate_entry` runs in
  place.
- **Node firmware:** updated over the LAN by OTA from the add-on, surfaced as an
  `update.aquila_<node>_firmware` entity per node — no internet, no cloud. The
  full design is in `firmware-ota.md`.
- **Contract compatibility:** the component compares its own
  `min_component_version` against `/api/v1/health`, and its API major against the
  canonical `api.major` object (`core-api.md` §4.1). A core that is
  too new or too old is reported clearly (repair/`ConfigEntryError`) rather than
  silently mismatched. Add-on and component version independently — they share
  the versioned API, not a version number.
- **Backup/restore:** HA snapshots include the add-on `/data` and the component
  config entry, so a full restore reconstitutes calibration, models and zones.

## 10. Open questions and deliberately deferred items

- **Single-repo, dual-store layout** (§3.3): whether one repository cleanly
  serves both the add-on store and HACS at once, or whether a second
  `belikh/aquila-addon` repository is cleaner. Packaging-only; choose at release
  scaffolding time.
- **Zeroconf discovery** of the core by the component (removes manual URL
  entry). Deferred; would also be the trigger to reconsider `host_network: true`
  if broadcast discovery is added.
- **Per-floor person count** and **per-zone presence confidence entities**:
  deferred until cross-floor fusion meets the accuracy bars.
- **Token auth**: the private-network/ingress trust boundary is the default
  credential (`core-api.md` §3); a bearer token is optional and used only for
  placements outside the add-on's private network. A token is never placed in a
  query string.
- **armv7/armhf**: unsupported by decision; no build, no claim.
