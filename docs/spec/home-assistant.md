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
- HA Core resolves the add-on by hostname = slug with `_` → `-`
  (`aquila_core` → `aquila-core`), so the component's default core URL is
  `http://aquila-core:8099`.

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
  retention_days: 30
schema:
  log_level: list(trace|debug|info|notice|warning|error|fatal)
  calibration_profile: str
  retention_days: int(1,365)
```

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
├── __init__.py          # async_setup_entry / async_unload_entry, panel registration
├── manifest.json
├── const.py             # DOMAIN, defaults, metric vocabulary
├── api.py               # aiohttp REST client (private HTTP API)
├── stream.py            # aiohttp WS client, reconnect/backoff, seq handling
├── coordinator.py       # AquilaCoordinator(DataUpdateCoordinator)
├── config_flow.py       # config + options + reauth
├── binary_sensor.py     # presence, occupancy, connectivity, self-test
├── sensor.py            # person count, breathing, heart rate, health
├── update.py            # per-node firmware update entity (see firmware-ota.md)
├── diagnostics.py       # config-entry diagnostics (redacted)
├── strings.json
├── translations/en.json
└── brand/icon.png       # custom-integration branding
```

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
  "iot_class": "local_push",
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
- `iot_class: local_push`: state arrives over the WS stream; the coordinator's
  HTTP poll is a slow reconciliation/health probe, not the primary source.
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
- **HACS:** `hacs.json` at the root with
  `{"name": "Aquila", "zip_release": true, "filename": "aquila.zip",
  "render_readme": true}`. Because the integration is not at the repo root,
  releases are shipped as a `aquila.zip` release asset containing
  `custom_components/aquila/`, produced by the release workflow. HACS extracts
  the asset, so the monorepo layout does not have to match the integration
  layout.
- If a single repo proves ambiguous for either store, the fallback is a separate
  `belikh/aquila-addon` repository; that is a packaging detail, not a
  behavioural one. Flagged in §10.

## 4. Naming convention

| Thing | Value | Rule |
| --- | --- | --- |
| Add-on slug | `aquila_core` | lowercase, `_` separators; full store slug `<repo>_aquila_core`; network hostname `aquila-core` |
| Component domain | `aquila` | drives `custom_components/aquila`, entity service namespace, `DOMAIN` |
| Add-on TCP port | `8099` | HTTP + WS + ingress (not host-published) |
| Add-on UDP ports | `9100`/`9101` | CSI ingest + node control (host-published; ADR-0003 §7) |
| Entity unique_id | `aquila:{instance_id}:{zone_id}:{metric}` | `instance_id` from core `/api/v1/health`; `zone_id` stable slug |
| Node device id | `("aquila", "{instance_id}:node:{node_mac}")` | one HA device per node |
| Zone device id | `("aquila", "{instance_id}:zone:{zone_id}")` | one HA device per room/floor |

- Component `DOMAIN = "aquila"`. The add-on slug is `aquila_core` (the core,
  not the whole product); there is exactly one add-on in v1. This slug, the
  derived `aquila-core` hostname and the `ghcr.io/belikh/aquila-core-*` image
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
  snapshot plus a sequenced event stream is safer and easier to migrate.
- **MQTT discovery would fight the entity model.** Aquila entities carry
  zone-scoped unique_ids, config subentries and diagnostics; building them from
  MQTT discovery payloads is fragile compared with a direct, typed snapshot.
- **Local push is sufficient.** The core and component are on the same host and
  the same Docker network. The cost MQTT avoids (a shared broker between
  unreliable remote clients) does not apply. The HA-recommended alternative for a
  same-host hub is a direct local API with a WS push, which is what we do.
- Non-goal reasoning: MQTT would also be the first step toward exposing Aquila
  state to arbitrary external consumers, which v1 explicitly forbids (no public
  API, in-tree only).

MQTT is not forbidden forever; it is simply the wrong seam for v1. If a future
multi-host topology appears, MQTT or the private API can be reconsidered behind
the same component.

### 5.2 HTTP + WS API (the actual contract)

Private, local, JSON, versioned. The core advertises its contract version; the
component refuses or degrades on mismatch (§9).

REST (all `GET` unless noted), base `http://aquila-core:8099`:

| Path | Purpose |
| --- | --- |
| `/api/v1/health` | `instance_id`, `api_version`, `min_component_version`, uptime, node counts, calibration state |
| `/api/v1/schema` | zone tree (home → floor → room), metric vocabulary, units |
| `/api/v1/state` | full current snapshot: every zone's presence/vitals + system health (coordinator poll + initial load) |
| `/api/v1/nodes` | per-node identity, MAC, floor/room, online state, clock-drift estimate |
| `/api/v1/firmware` | firmware image store manifest (versions, sha256, target chip) — see `firmware-ota.md` §7 |
| `/api/v1/nodes/{node}/firmware` | installed version, boot partition, pending state, last OTA result |
| `/api/v1/nodes/{node}/firmware/install` (`POST`) | begin a firmware OTA for a node |
| `/api/v1/nodes/{node}/firmware/job` | OTA job progress/state |
| `/api/v1/nodes/{node}/firmware/abort` (`POST`) | abort an in-progress OTA |

WS: `ws://aquila-core:8099/api/v1/stream`

- Server → client events, each carrying a monotonic `seq` and a `ts`:
  `{"type":"state","seq":123,"ts":...,"zone_id":"living_room",
    "measurements":{"presence":true,"breathing_rate":13.2,"heart_rate":62}}`
  and `{"type":"health","seq":124,"ts":...,"core_connected":true,"nodes_online":9}`.
- A `firmware` event carries per-node OTA progress for the `update` entity:
  `{"type":"firmware","seq":125,"ts":...,"node":"aa:bb:...","state":"downloading","progress":0.42}`
  (see `firmware-ota.md` §7).
- The component ignores any event whose `seq` is not greater than the last
  applied `seq`; timestamps are informational only (software time sync is the
  core's concern, not HA's).
- **Auth:** since 8099 is not published to the LAN, the default is trusted
  local access with **no token**. If a deployment ever exposes the API beyond
  the `hassio` network, a `X-Aquila-Token` bearer is required and is set in the
  config flow. Default-off auth is honest about the trust boundary rather than
  pretending a token adds security on an unpublished port.

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

### 6.2 Coordinator and streaming

```python
class AquilaCoordinator(DataUpdateCoordinator[dict]):
    """Caches the latest core snapshot; WS push is primary, HTTP poll reconciles."""

    def __init__(self, hass, client, stream, update_interval):
        super().__init__(hass, _LOGGER, name=DOMAIN, update_interval=update_interval)
        self._client = client
        self._stream = stream
        self._seq = 0

    async def _async_update_data(self) -> dict:
        # slow reconciliation / health probe; also the initial load
        return await self._client.async_get_state()

    def apply_event(self, event: dict) -> None:
        if event["seq"] <= self._seq:      # stale, ignore
            return
        self._seq = event["seq"]
        self.data = merge(self.data, event)  # partial merge, keep untouched zones
        self.async_set_updated_data(self.data)
```

- `_async_update_data` performs `GET /api/v1/state`; the interval is slow
  (default 60 s, configurable). This is the reconciliation and health path.
- A separate `AquilaStreamClient` owns the WS connection with reconnect and
  exponential backoff; on each message it calls `coordinator.apply_event`. A WS
  drop sets `core_connected = false` via the health snapshot and flips
  `coordinator.last_update_success`, which marks entities unavailable.
- Entities are `CoordinatorEntity` subclasses with `_attr_should_poll = False`;
  they read from `coordinator.data` and never hold their own transport.
- On setup failure to reach the core, the config entry raises
  `ConfigEntryNotReady`; on repeated poll failure `UpdateFailed` drives HA's
  entity-availability handling.

## 7. Config flow

`config_flow.py` implements `ConfigFlow` (domain `aquila`):

- `async_step_user`: one field, **Core URL**, defaulted to
  `http://aquila-core:8099` (the add-on hostname). Validates by calling
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

Two layers, intentionally:

1. **Add-on ingress panel (baseline, zero-component-cost).** Because the add-on
   sets `ingress: true` with `panel_icon`/`panel_title`, HA automatically adds a
   **management** sidebar entry (admin-only via `panel_admin: true`) pointing at
   the ingress dashboard. This works even before the HACS component is
   installed.
2. **Component panel (integration-owned).** In `async_setup`, the component
   registers a `panel_custom` after `async_setup_entry`, gated on admin and on
   `hass.data[DOMAIN]` being ready:

   ```python
   from homeassistant.components import panel_custom
   panel_custom.async_register_panel(
       hass, webcomponent_path="aquila-panel.js",
       frontend_url_path="aquila", sidebar_title="Aquila",
       sidebar_icon="mdi:radar", config={"instance_id": instance_id},
       admin_only=True, require_admin=True,
   )
   ```

   The web component frames core status (connection, node count, calibration
   state) from the component itself and deep-links to the ingress dashboard for
   the full management UI. The component only registers the panel once for all
   cases of the domain; `async_unload_entry` removes it when the last entry is
   unloaded, guarded by `hass.data[DOMAIN]` ref-counting.

The split keeps the heavy Rust/WASM UI in the add-on (single source of truth,
updates with the core) while the component owns the HA-native, admin-gated
navigation entry and the connection health shown to HA users.

## 9. Install and upgrade flow

**Install (fresh host):**

1. Add the add-on repository `https://github.com/belikh/aquila`;
   install **Aquila Core**; set options (`log_level`, `calibration_profile`,
   `retention_days`); start it. On first start the core generates `instance_id`,
   initialises `/data` (ledger, calibration store, drift state, model cache),
   binds UDP `9100`/`9101` and serves ingress on `8099`.
2. Provision nodes with the core's host address and UDP port (unicast).
3. In HACS, add `https://github.com/belikh/aquila` as a custom repository
   (integration); download **Aquila**; restart HA.
4. Settings → Devices & Services → Add Integration → **Aquila**. The URL
   defaults to `http://aquila-core:8099`; the flow validates `/api/v1/health`
   and creates the entry. Entities and the sidebar panel appear.

**Upgrade:**

- **Add-on:** bump `version:` in `aquila_core/config.yaml`; HA shows an update;
  the user clicks Update. `/data` persists; the core runs its own schema
  migrations on start. The dashboard (WASM) updates as part of this step.
- **Component:** bump `version` in `manifest.json`; the release workflow
  rebuilds `aquila.zip`; HACS shows an update; the user updates and restarts.
  `async_migrate_entry` runs in place.
- **Node firmware:** updated over the LAN by OTA from the add-on, surfaced as an
  `update.aquila_<node>_firmware` entity per node — no internet, no cloud. The
  full design is in `firmware-ota.md`.
- **Contract compatibility:** the component compares its own
  `min_component_version`/`api_version` against `/api/v1/health`. A core that is
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
- **Token auth**: default-off because 8099 is unpublished; revisit only if a
  non-`hassio` deployment is ever supported.
- **armv7/armhf**: unsupported by decision; no build, no claim.
