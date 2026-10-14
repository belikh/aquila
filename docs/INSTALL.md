# Installing and upgrading Aquila

Status: install guide for the v1 reference deployment — a Linux host running
Home Assistant OS (or Supervised), with ESP32-S3 nodes on the same LAN.

This is the operational companion to `docs/spec/home-assistant.md` §9. It
describes three things that ship separately and version independently:

1. **Aquila Core** — the Home Assistant add-on (App): the Rust core plus the
   management dashboard.
2. **Aquila** — the HACS custom component (Python): projects the core's sensing
   results into Home Assistant entities.
3. **Node firmware** — the ESP32-S3 CSI firmware, updated over the LAN by OTA.

> **Honest status.** The packaging and the install path are prepared and
> inspectable. The v1 accuracy bars are **not yet measured** — they are
> hardware-gated — so this is not a "v1.0.0" release. See the "v1 status"
> section in the top-level `README.md` and the claims table in
> `docs/README.md`.

---

## Prerequisites

- Home Assistant OS or Supervised on `amd64` or `aarch64` (Raspberry Pi 4/5
  64-bit, Yellow/Green). `armv7`/`armhf` (32-bit) is unsupported by decision
  (`home-assistant.md` §3.1).
- The host and the nodes on the same LAN. The nodes send raw CSI over UDP to
  the add-on; the add-on runs with bridge networking and publishes UDP `9100`
  (CSI ingest) and UDP `9101` (node control). The HTTP+WS API on `8099` is
  **not** exposed to the LAN; Home Assistant reaches it over the internal
  `hassio` network at `http://aquila-core:8099`.
- One or more ESP32-S3 nodes (reference: a module with WiFi, e.g.
  ESP32-S3-DevKitC-1). See `firmware/esp32-s3-csi-node/README.md`.

---

## 1. Install the Aquila Core add-on

1. In Home Assistant: **Settings → Add-ons → Add-on Store**.
2. Top-right **⋮ → Repositories**, add
   `https://github.com/belikh/aquila`, and close the dialog.
3. **Aquila Core** now appears. Open it and click **Install**.
   - The default distribution path is a prebuilt multi-arch image on GHCR
     (`ghcr.io/belikh/aquila-core-{arch}`), published by
     `.github/workflows/release.yml` on a `v*` tag.
   - If no image is published for your arch yet, use the local-build fallback:
     the **Build** option triggers `aquila_core/Dockerfile` via
     `aquila_core/build.yaml`. The Rust + WASM toolchain is heavy; expect a long
     build.
4. On the **Configuration** tab, set options (or leave the defaults):
   - `log_level` — `info` by default.
   - `calibration_profile` — `default` by default.
   - `retention_days` — defaults to `0`, meaning **retain indefinitely**. A
     positive value is an explicit opt-in to automatic expiry; the shipped
     default never deletes data on its own.
5. **Start** the add-on. On first start the core generates its `instance_id`,
   initialises its persistent state under `/data`, and serves the dashboard on
   `8099`.
6. Confirm it is up: **Settings → Add-ons → Aquila Core → Open Web UI** (or the
   **Aquila** entry in the sidebar). This is the management dashboard served over
   sidebar ingress; it works before the HACS component is installed.

---

## 2. Install the HACS custom component

The component is a HACS **integration**. It is not at the repository root, so
releases ship `aquila.zip` (containing `custom_components/aquila/`) as a release
asset (`hacs.json` sets `zip_release: true`, `filename: aquila.zip`).

1. Install [HACS](https://hacs.xyz) if it is not already installed.
2. In HACS: **⋮ → Custom repositories**, add
   `https://github.com/belikh/aquila` with category **Integration**.
3. Search for **Aquila**, open it, and **Download** the latest release.
4. **Restart Home Assistant**.
5. **Settings → Devices & Services → Add Integration → Aquila.** The core URL
   defaults to `http://aquila-core:8099` (the add-on hostname). The flow
   validates `GET /api/v1/health` and creates the config entry. Entities and the
   add-on's sidebar panel appear.

Options (`update_interval`, `expose_floors`, `expose_room_vitals`) are under
**Configure** on the integration. A bearer token is only needed if the core is
placed outside the add-on's private network; a token is never placed in a query
string.

---

## 3. Provision the nodes

This step needs the physical nodes and is **hardware-gated** in this repository:
the firmware compiles (see below) but flashing and on-device behaviour are
unverified here.

1. Build and flash the reference firmware, following
   `firmware/esp32-s3-csi-node/README.md`:
   ```
   nix develop .#esp --command bash -c \
     'cd firmware/esp32-s3-csi-node && idf.py set-target esp32s3 && idf.py build && idf.py flash monitor'
   ```
   (Flashing needs a connected board; `idf.py flash` is not run in CI.)
2. Configure each node with `idf.py menuconfig` (Kconfig project settings):
   the WiFi SSID/password, the **core endpoint** (the HA host's LAN address and
   UDP port `9100`), a unique **node id**, and the **pairing code** used to
   authenticate the node's control channel (`capture-and-firmware.md` §6.3).
3. Power the nodes and place them per your topology (one or more per room). CSI
   is sent as **unicast** UDP to the core; there is no broadcast discovery and no
   ESP-NOW (`home-assistant.md` §2).
4. In the dashboard, confirm the nodes appear and are online. Pair each node and
   assign it to a room (`aquila-api` node resources; the dashboard also edits the
   topology).

---

## 4. First calibration

1. Open the **Aquila** dashboard (sidebar entry, admin-only).
2. Define the home → floor → room → node topology.
3. Trigger a **global calibration** pass. Calibration is a versioned,
   commit-keyed artefact, whole-home by default; it removes per-node and
   per-session offsets before sensing runs.
4. Confirm the calibration artefact is visible and versioned, and that
   `sensor.aquila_calibration_state` leaves `uncalibrated`.

Until calibration has run, zone presence and vitals may be unavailable rather
than guessed — a `null` reading maps to HA `unavailable`/`unknown`, never a
fabricated zero (`home-assistant.md` §6.1).

---

## 5. Update node firmware (OTA)

Node firmware is updated over the LAN from the add-on — no internet and no
cloud. The core holds a signed image store and writes to the node's inactive
A/B partition; the node confirms the image after a health window or rolls back
automatically (`firmware-ota.md`).

1. Each node appears as an `update.aquila_<node>_firmware` entity.
2. Install the desired firmware version from the entity in Home Assistant, or
   from the dashboard. Progress and the last OTA result are visible on the node's
   firmware resource.

There is no public REST/WS/MCP surface and the node runs no HTTP server; OTA
runs entirely on the private control channel.

---

## Upgrading

- **Add-on:** bump `version:` in `aquila_core/config.yaml`, publish a new tag;
  Home Assistant offers an update. Click **Update**. `/data` persists and the
  core runs its own schema migrations on start. The dashboard (WASM) is baked
  into the image, so it updates with the add-on.
- **Component:** bump `version` in `custom_components/aquila/manifest.json`; the
  release workflow rebuilds `aquila.zip`; HACS offers an update. Update and
  restart; `async_migrate_entry` runs in place.
- **Node firmware:** OTA via the `update` entity (above).
- **Contract compatibility:** the component compares its version against the
  core's `/api/v1/health`; an incompatible pair is reported clearly rather than
  silently mismatched. The add-on and the component version independently and
  meet at the versioned private API.

## Backup and restore

Home Assistant snapshots include the add-on's `/data` volume (calibration store,
the commit-keyed ledger, per-node drift state, downloaded model bundles) and the
component's config entry, so a full restore reconstitutes calibration, models and
zones (`home-assistant.md` §9).

## Uninstalling

Remove the component from HACS and restart, then uninstall the **Aquila Core**
add-on. Uninstalling the add-on removes its `/data` volume — export or snapshot
first if you need the calibration store or the ledger.

---

## Troubleshooting

- **The component cannot reach the core.** It defaults to
  `http://aquila-core:8099`; confirm the add-on is running and that the hostname
  resolves on the `hassio` network. If you changed the add-on slug, update the
  core URL accordingly.
- **No CSI arriving.** Confirm the nodes are provisioned with the HA host's LAN
  address and UDP `9100`, and that UDP `9100` is published by the add-on. CSI is
  unicast; there is no discovery.
- **A node never appears.** Check its node id and pairing code, and that it
  joined WiFi.

For the full deployment model see `docs/spec/home-assistant.md`; for the wire
and firmware see `docs/spec/capture-and-firmware.md` and
`docs/spec/firmware-ota.md`.
