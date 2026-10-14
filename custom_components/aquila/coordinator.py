"""Coordinator, shared entity base and snapshot helpers for Aquila.

``AquilaCoordinator`` caches the latest core snapshot. v1 has no WebSocket stream
(``core-api.md`` §10), so the HTTP poll is the primary source and the health
probe (home-assistant.md §6.2).

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
from .const import DOMAIN, MANUFACTURER, ZONE_KIND_HOME
from .stream import async_collect_snapshot


_LOGGER = logging.getLogger(__name__)


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


class AquilaCoordinator(DataUpdateCoordinator[dict[str, Any]]):
    """Cache the latest core snapshot; the HTTP poll is the v1 source."""

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

    async def _async_update_data(self) -> dict[str, Any]:
        """Poll the canonical snapshot endpoints (home-assistant.md §6.2).

        ``GET /api/v1/state`` is required; nodes and the firmware manifest are
        fetched best-effort by :func:`async_collect_snapshot`. A failure raises
        ``UpdateFailed`` so HA marks entities unavailable until the next poll.
        """
        try:
            return await async_collect_snapshot(self._client)
        except AquilaAuthError as err:
            self._trigger_reauth()
            raise UpdateFailed("Authentication with the Aquila core failed") from err
        except AquilaError as err:
            raise UpdateFailed(str(err)) from err

    def _trigger_reauth(self) -> None:
        """Start the reauth flow when the core rejects the token (home-assistant.md §7)."""
        if self._entry is None:
            return
        if self._entry.state is not ConfigEntryState.LOADED:
            return
        with suppress(Exception):
            self.hass.async_create_task(self._entry.async_start_reauth(self.hass))

    def home_zone_id(self) -> str | None:
        """Return the zone id of the home-level zone, if present."""
        for zone_id, zone in (self.data or {}).get("zones", {}).items():
            if zone.get("kind") == ZONE_KIND_HOME:
                return zone_id
        return None


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
        """Available only while the last poll succeeded."""
        return self.coordinator.last_update_success

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
