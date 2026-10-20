"""Binary sensor platform for Aquila.

Entities: home occupancy, per-zone presence, core connectivity and self-test
(home-assistant.md §6.1). ``null`` readings map to ``unknown``, never a
fabricated value.
"""

from __future__ import annotations

import logging

from homeassistant.components.binary_sensor import (
    BinarySensorDeviceClass,
    BinarySensorEntity,
)
from homeassistant.config_entries import ConfigEntry
from homeassistant.const import EntityCategory
from homeassistant.core import HomeAssistant, callback
from homeassistant.helpers.entity_platform import AddEntitiesCallback

from .const import (
    DOMAIN,
    HEALTH_CORE_CONNECTED,
    HEALTH_SELF_TEST_OK,
    METRIC_CORE_CONNECTED,
    METRIC_OCCUPANCY,
    METRIC_PRESENCE,
    METRIC_SELF_TEST,
    SYSTEM_ZONE_ID,
    ZONE_KIND_FLOOR,
    ZONE_KIND_HOME,
    ZONE_KIND_ROOM,
)
from .coordinator import (
    AquilaCoordinator,
    AquilaEntity,
    AquilaRuntimeData,
    core_device_info,
    zone_device_info,
)

_LOGGER = logging.getLogger(__name__)


async def async_setup_entry(
    hass: HomeAssistant,
    entry: ConfigEntry,
    async_add_entities: AddEntitiesCallback,
) -> None:
    """Set up Aquila binary sensors."""
    runtime: AquilaRuntimeData = hass.data[DOMAIN][entry.entry_id]
    coordinator = runtime.coordinator

    # Static, system-scoped diagnostics always exist.
    async_add_entities(
        [
            AquilaCoreConnectedBinarySensor(coordinator, runtime),
            AquilaSelfTestBinarySensor(coordinator, runtime),
        ]
    )

    known: set[str] = set()

    @callback
    def _discover() -> None:
        new_entities: list[BinarySensorEntity] = []
        zones = (coordinator.data or {}).get("zones", {}) or {}

        home = _find_home_zone(zones)
        if home is not None and METRIC_OCCUPANCY not in known:
            known.add(METRIC_OCCUPANCY)
            new_entities.append(AquilaOccupancyBinarySensor(coordinator, runtime, *home))

        for zone_id, zone in zones.items():
            kind = zone.get("kind")
            if kind not in (ZONE_KIND_ROOM, ZONE_KIND_FLOOR):
                continue
            if kind == ZONE_KIND_FLOOR and not runtime.expose_floors:
                continue
            key = f"{METRIC_PRESENCE}:{zone_id}"
            if key in known:
                continue
            known.add(key)
            new_entities.append(
                AquilaPresenceBinarySensor(coordinator, runtime, zone_id, zone)
            )

        if new_entities:
            async_add_entities(new_entities)

    _discover()
    entry.async_on_unload(coordinator.async_add_listener(_discover))


def _find_home_zone(zones: dict) -> tuple[str, dict] | None:
    for zone_id, zone in zones.items():
        if zone.get("kind") == ZONE_KIND_HOME:
            return zone_id, zone
    return None


class AquilaOccupancyBinarySensor(AquilaEntity, BinarySensorEntity):
    """Home-level "is anyone home" aggregate."""

    _attr_device_class = BinarySensorDeviceClass.OCCUPANCY
    _attr_translation_key = METRIC_OCCUPANCY

    def __init__(
        self,
        coordinator: AquilaCoordinator,
        runtime: AquilaRuntimeData,
        zone_id: str,
        zone: dict,
    ) -> None:
        super().__init__(
            coordinator,
            runtime,
            unique_id=f"aquila:{runtime.instance_id}:{zone_id}:{METRIC_OCCUPANCY}",
            device_info=core_device_info(runtime),
        )
        self._zone_id = zone_id

    @property
    def is_on(self) -> bool | None:
        zone = (self.coordinator.data or {}).get("zones", {}).get(self._zone_id, {})
        value = zone.get("presence")
        return None if value is None else bool(value)


class AquilaPresenceBinarySensor(AquilaEntity, BinarySensorEntity):
    """Per-zone presence (rooms and floors)."""

    _attr_device_class = BinarySensorDeviceClass.PRESENCE
    _attr_translation_key = METRIC_PRESENCE

    def __init__(
        self,
        coordinator: AquilaCoordinator,
        runtime: AquilaRuntimeData,
        zone_id: str,
        zone: dict,
    ) -> None:
        super().__init__(
            coordinator,
            runtime,
            unique_id=f"aquila:{runtime.instance_id}:{zone_id}:{METRIC_PRESENCE}",
            device_info=zone_device_info(runtime, zone_id, zone),
        )
        self._zone_id = zone_id

    @property
    def is_on(self) -> bool | None:
        zone = (self.coordinator.data or {}).get("zones", {}).get(self._zone_id, {})
        value = zone.get("presence")
        return None if value is None else bool(value)


class AquilaCoreConnectedBinarySensor(AquilaEntity, BinarySensorEntity):
    """Diagnostic: is the core reachable over the private API."""

    _attr_device_class = BinarySensorDeviceClass.CONNECTIVITY
    _attr_entity_category = EntityCategory.DIAGNOSTIC
    _attr_translation_key = METRIC_CORE_CONNECTED

    def __init__(
        self,
        coordinator: AquilaCoordinator,
        runtime: AquilaRuntimeData,
    ) -> None:
        super().__init__(
            coordinator,
            runtime,
            unique_id=f"aquila:{runtime.instance_id}:{SYSTEM_ZONE_ID}:{METRIC_CORE_CONNECTED}",
            device_info=core_device_info(runtime),
        )

    @property
    def is_on(self) -> bool | None:
        value = self.health.get(HEALTH_CORE_CONNECTED)
        if value is None:
            return self.coordinator.last_update_success
        return bool(value) and self.coordinator.last_update_success


class AquilaSelfTestBinarySensor(AquilaEntity, BinarySensorEntity):
    """Diagnostic: core self-test failure (``problem`` device class)."""

    _attr_device_class = BinarySensorDeviceClass.PROBLEM
    _attr_entity_category = EntityCategory.DIAGNOSTIC
    _attr_translation_key = METRIC_SELF_TEST

    def __init__(
        self,
        coordinator: AquilaCoordinator,
        runtime: AquilaRuntimeData,
    ) -> None:
        super().__init__(
            coordinator,
            runtime,
            unique_id=f"aquila:{runtime.instance_id}:{SYSTEM_ZONE_ID}:{METRIC_SELF_TEST}",
            device_info=core_device_info(runtime),
        )

    @property
    def is_on(self) -> bool | None:
        value = self.health.get(HEALTH_SELF_TEST_OK)
        if value is None:
            return None
        return not bool(value)
