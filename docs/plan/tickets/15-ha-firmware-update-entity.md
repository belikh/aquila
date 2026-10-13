# 14 — Home Assistant firmware update entity

- Type: task
- Blocked by: 08, 12
- Status: open

## Goal

Surface per-node firmware updates in Home Assistant: one `update` entity per node
with installed/latest versions and progress, plus install and abort, driven by the
core API — the "home-assistant update functionality".

## Implements

- `docs/spec/firmware-ota.md` §7.
- `docs/spec/home-assistant.md` §5 (endpoints), §6 (entity list + `update.py`),
  §9 (upgrade flow).
- `custom_components/aquila/update.py`.

## Relies on

- `docs/reference/sources.json`: `ha-update`, `hacs`, `ha-arch`.

## Acceptance criteria

- An `update` platform maps each node to `update.aquila_<node>_firmware` with
  `installed_version`, `latest_version`, `update_available`, `in_progress`,
  `update_percentage`, and an install action.
- The coordinator merges the `firmware` WS events and `/api/v1/nodes/{node}/firmware`
  into the snapshot; entities are `CoordinatorEntity` subclasses.
- An `aquila.install_firmware` service triggers `POST .../firmware/install`.
- `manifest.json`, `strings.json` and translations updated; diagnostics redact
  nothing new (firmware versions are not sensitive).
- Dashboard OTA panel is optional in this ticket; if deferred, say so.

## First test-first step

Add firmware state to the coordinator snapshot and a failing test that maps it to
an `UpdateEntity` per node.
