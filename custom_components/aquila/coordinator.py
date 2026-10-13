"""Coordinator, shared entity base and snapshot helpers for Aquila.

``AquilaCoordinator`` caches the latest core snapshot. The WS push is the
primary source; the HTTP poll is a slow reconciliation and health probe
(home-assistant.md §6.2).

The shared ``AquilaEntity`` base and the device-info builders live here rather
than in a separate module so the component file set matches
``docs/spec/home-assistant.md`` §3.2 exactly.
"""

from __future__ import annotations

import logging
from contextlib import suppress
from dataclasses import dataclass
from datetime import timedelta
from typing import Any

from homeassistant.config_entries import ConfigEntry, ConfigEntryState
from homeassistant.core import HomeAssistant
from homeassistant.helpers.device_registry import DeviceInfo
from homeassistant.helpers.update_coordinator import (
    CoordinatorEntity,
    DataUpdateCoordinator,
    UpdateFailed,
)

from .api import AquilaApiClient, AquilaAuthError, AquilaError
from .const import (
    DOMAIN,
    EVENT_TYPE_FIRMWARE,
    EVENT_TYPE_HEALTH,
    EVENT_TYPE_STATE,
    HEALTH_CALIBRATION_STATE,
    HEALTH_CLOCK_DRIFT_MS,
    HEALTH_CORE_CONNECTED,
    HEALTH_LAST_PACKET_TS,
    HEALTH_NODES_ONLINE,
    HEALTH_NODES_TOTAL,
    HEALTH_SELF_TEST_OK,
    MANUFACTURER,
    ZONE_KIND_HOME,
)

_LOGGER = logging.getLogger(__name__)

# Health fields carried by a ``health`` stream event and folded into the
# snapshot (home-assistant.md §5.2/§6).
_HEALTH_FIELDS = (
    HEALTH_CORE_CONNECTED,
    HEALTH_NODES_ONLINE,
    HEALTH_NODES_TOTAL,
    HEALTH_CALIBRATION_STATE,
    HEALTH_CLOCK_DRIFT_MS,
    HEALTH_LAST_PACKET_TS,
    HEALTH_SELF_TEST_OK,
)

# Firmware fields carried by a ``firmware`` stream event (firmware-ota.md §7).
_FIRMWARE_EVENT_FIELDS = ("state", "progress")


@dataclass
class AquilaRuntimeData:
    """Per-config-entry runtime state held in ``hass.data[DOMAIN]``."""

    client: AquilaApiClient
    coordinator: "AquilaCoordinator"
    instance_id: str
    api_version: str
    min_component_version: str | None
    update_interval: int
    expose_floors: bool
    expose_room_vitals: bool
    stream: Any = None


class AquilaCoordinator(DataUpdateCoordinator[dict[str, Any]]):
    """Cache the latest core snapshot; WS push is primary, HTTP poll reconciles."""

    def __init__(
        self,
        hass: HomeAssistant,
        client: AquilaApiClient,
        update_interval: int,
        instance_id: str,
        entry: ConfigEntry | None = None,
    ) -> None:
        super().__init__(
            hass,
            _LOGGER,
            name=DOMAIN,
            update_interval=timedelta(seconds=update_interval),
        )
        self._client = client
        self._entry = entry
        self.instance_id = instance_id
        self._seq = 0
        #: Whether the WS stream currently has a live connection.
        self.stream_connected = False

    async def _async_update_data(self) -> dict[str, Any]:
        """Reconcile via ``GET /api/v1/state``; also the initial load.

        Node identity and the firmware manifest are fetched best-effort and
        merged under ``nodes``/``firmware`` so the ``update`` platform has data
        (firmware-ota.md §7). Their absence never fails the reconciliation.
        """
        try:
            state = await self._client.async_get_state()
        except AquilaAuthError as err:
            self._trigger_reauth()
            raise UpdateFailed("Authentication with the Aquila core failed") from err
        except AquilaError as err:
            raise UpdateFailed(str(err)) from err

        snapshot: dict[str, Any] = dict(state) if isinstance(state, dict) else {}
        snapshot.setdefault("zones", {})
        snapshot.setdefault("health", {})

        try:
            nodes = await self._client.async_get_nodes()
        except AquilaError as err:
            _LOGGER.debug("Node reconciliation failed: %s", err)
        else:
            snapshot["nodes"] = _normalise_nodes(nodes)

        try:
            snapshot["firmware_manifest"] = await self._client.async_get_firmware_manifest()
        except AquilaError as err:
            _LOGGER.debug("Firmware manifest reconciliation failed: %s", err)

        # Preserve stream-derived fields across a poll: a poll must not claim
        # the core is connected when the WS is down.
        snapshot["health"].setdefault(
            HEALTH_CORE_CONNECTED, self.stream_connected
        )
        return snapshot

    def apply_event(self, event: dict[str, Any]) -> None:
        """Apply one stream event to the cached snapshot (home-assistant.md §6.2)."""
        seq = event.get("seq")
        if isinstance(seq, int):
            if seq <= self._seq:
                return  # stale, ignore
            self._seq = seq

        if self.data is None:
            self.data = {"zones": {}, "health": {}}

        event_type = event.get("type")
        if event_type == EVENT_TYPE_STATE:
            self._apply_state_event(event)
        elif event_type == EVENT_TYPE_HEALTH:
            self._apply_health_event(event)
        elif event_type == EVENT_TYPE_FIRMWARE:
            self._apply_firmware_event(event)
        else:
            return

        self.async_set_updated_data(self.data)

    def _apply_state_event(self, event: dict[str, Any]) -> None:
        zone_id = event.get("zone_id")
        if not zone_id:
            return
        measurements = event.get("measurements")
        if not isinstance(measurements, dict):
            return
        zones = self.data.setdefault("zones", {})
        zone = zones.setdefault(zone_id, {})
        zone.update(measurements)  # partial merge; untouched zones are kept

    def _apply_health_event(self, event: dict[str, Any]) -> None:
        health = self.data.setdefault("health", {})
        for field in _HEALTH_FIELDS:
            if field in event:
                health[field] = event[field]

    def _apply_firmware_event(self, event: dict[str, Any]) -> None:
        node = event.get("node")
        if not node:
            return
        nodes = self.data.setdefault("nodes", {})
        node_state = nodes.setdefault(node, {"mac": node})
        firmware = node_state.setdefault("firmware", {})
        state = event.get("state")
        if state is not None:
            firmware["state"] = state
            firmware["in_progress"] = state not in ("idle", "none", "complete", "failed")
        progress = event.get("progress")
        if isinstance(progress, (int, float)):
            firmware["update_percentage"] = float(progress) * 100.0

    def reset_seq(self) -> None:
        """Reset the applied sequence on a new WS connection.

        ``seq`` is per-connection (core-api.md §10.2); without this reset the
        first frames of a reconnected stream would be dropped as stale and a gap
        would be misread.
        """
        self._seq = 0

    def _trigger_reauth(self) -> None:
        """Start the reauth flow when the core rejects the token (home-assistant.md §7)."""
        if self._entry is None:
            return
        if self._entry.state is not ConfigEntryState.LOADED:
            return
        with suppress(Exception):
            self.hass.async_create_task(self._entry.async_start_reauth(self.hass))

    def set_stream_connected(self, connected: bool) -> None:
        """Record stream availability and reflect it in the snapshot.

        A WS drop writes ``core_connected = false`` and flips
        ``last_update_success`` so entities become unavailable; reconnect
        restores both (home-assistant.md §6.2).
        """
        self.stream_connected = connected
        if self.data is not None:
            self.data.setdefault("health", {})[HEALTH_CORE_CONNECTED] = connected
            self.async_update_listeners()
        self.last_update_success = connected

    def home_zone_id(self) -> str | None:
        """Return the zone id of the home-level zone, if present."""
        for zone_id, zone in (self.data or {}).get("zones", {}).items():
            if zone.get("kind") == ZONE_KIND_HOME:
                return zone_id
        return None


def _normalise_nodes(payload: Any) -> dict[str, Any]:
    """Normalise the ``/api/v1/nodes`` payload into a MAC-keyed mapping."""
    if isinstance(payload, dict):
        items = payload.get("items")
        if isinstance(items, list):
            raw_nodes = items
        else:
            # Already MAC-keyed.
            return {str(k): v for k, v in payload.items() if isinstance(v, dict)}
    elif isinstance(payload, list):
        raw_nodes = payload
    else:
        return {}

    nodes: dict[str, Any] = {}
    for node in raw_nodes:
        if not isinstance(node, dict):
            continue
        mac = node.get("mac") or node.get("id")
        if mac:
            nodes[str(mac)] = node
    return nodes


class AquilaEntity(CoordinatorEntity[AquilaCoordinator]):
    """Shared base for Aquila entities: no polling, availability from the core."""

    _attr_has_entity_name = True
    _attr_should_poll = False

    def __init__(
        self,
        coordinator: AquilaCoordinator,
        runtime: AquilaRuntimeData,
        *,
        unique_id: str,
        device_info: DeviceInfo,
    ) -> None:
        super().__init__(coordinator)
        self._runtime = runtime
        self._attr_unique_id = unique_id
        self._attr_device_info = device_info

    @property
    def available(self) -> bool:
        """Available only while the poll succeeds and the stream is connected."""
        return self.coordinator.last_update_success and self.coordinator.stream_connected

    @property
    def health(self) -> dict[str, Any]:
        """The current health block of the snapshot."""
        return (self.coordinator.data or {}).get("health", {}) or {}


def core_device_info(runtime: AquilaRuntimeData) -> DeviceInfo:
    """Device info for the Aquila hub device.

    The hub carries the core/system entities *and* the home-level aggregates
    (occupancy, person count, home vitals). Its name is ``Aquila`` so that
    Home Assistant's default object-id derivation yields the spec ids
    (``binary_sensor.aquila_core_connected``, ``binary_sensor.aquila_occupancy``)
    from the translation names (home-assistant.md §4/§6.1).
    """
    info = DeviceInfo(
        identifiers={(DOMAIN, runtime.instance_id)},
        name="Aquila",
        manufacturer=MANUFACTURER,
        model="Aquila Core",
    )
    if runtime.api_version:
        info["sw_version"] = runtime.api_version
    return info


def zone_device_info(
    runtime: AquilaRuntimeData,
    zone_id: str,
    zone: dict[str, Any],
) -> DeviceInfo:
    """Device info for one floor/room zone device (home-assistant.md §4).

    Named ``Aquila <zone>`` so the entity id derives as
    ``binary_sensor.aquila_<zone>_presence`` (home-assistant.md §4).
    """
    name = zone.get("name") or zone_id
    return DeviceInfo(
        identifiers={(DOMAIN, f"{runtime.instance_id}:zone:{zone_id}")},
        name=f"Aquila {name}",
        manufacturer=MANUFACTURER,
        model=(zone.get("kind") or "zone").capitalize(),
        via_device=(DOMAIN, runtime.instance_id),
    )


def node_device_info(runtime: AquilaRuntimeData, node: dict[str, Any]) -> DeviceInfo:
    """Device info for one node device (home-assistant.md §4).

    Named ``Aquila <node>`` so the entity id derives as
    ``update.aquila_<node>_firmware`` (firmware-ota.md §7).
    """
    mac = str(node.get("mac") or node.get("id"))
    name = node.get("name") or mac
    return DeviceInfo(
        identifiers={(DOMAIN, f"{runtime.instance_id}:node:{mac}")},
        name=f"Aquila {name}",
        manufacturer=MANUFACTURER,
        model=node.get("hardware") or "ESP32-S3 node",
        via_device=(DOMAIN, runtime.instance_id),
    )
