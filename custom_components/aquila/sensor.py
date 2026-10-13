"""Sensor platform for Aquila.

Entities: home person count, per-room/home vitals, and system diagnostics
(home-assistant.md §6.1). A ``null`` reading maps to ``unknown``/``unavailable``
and is never replaced with zero.
"""

from __future__ import annotations

import logging
from typing import Any

from homeassistant.components.sensor import (
    SensorDeviceClass,
    SensorEntity,
    SensorStateClass,
)
from homeassistant.config_entries import ConfigEntry
from homeassistant.const import EntityCategory
from homeassistant.core import HomeAssistant, callback
from homeassistant.helpers.device_registry import DeviceInfo
from homeassistant.helpers.entity_platform import AddEntitiesCallback
from homeassistant.util import dt as dt_util

from .const import (
    CALIBRATION_STATES,
    DOMAIN,
    HEALTH_CALIBRATION_STATE,
    HEALTH_CLOCK_DRIFT_MS,
    HEALTH_LAST_PACKET_TS,
    HEALTH_NODES_ONLINE,
    HEALTH_NODES_TOTAL,
    METRIC_BREATHING_RATE,
    METRIC_CALIBRATION_STATE,
    METRIC_CLOCK_DRIFT,
    METRIC_HEART_RATE,
    METRIC_LAST_CSI_PACKET,
    METRIC_NODES_ONLINE,
    METRIC_NODES_TOTAL,
    METRIC_PERSON_COUNT,
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

#: Vitals are emitted per room and aggregated to home (home-assistant.md §6.1).
_VITAL_METRICS = {
    METRIC_BREATHING_RATE: "brpm",
    METRIC_HEART_RATE: "bpm",
}


async def async_setup_entry(
    hass: HomeAssistant,
    entry: ConfigEntry,
    async_add_entities: AddEntitiesCallback,
) -> None:
    """Set up Aquila sensors."""
    runtime: AquilaRuntimeData = hass.data[DOMAIN][entry.entry_id]
    coordinator = runtime.coordinator

    async_add_entities(
        [
            AquilaNodesOnlineSensor(coordinator, runtime),
            AquilaNodesTotalSensor(coordinator, runtime),
            AquilaCalibrationStateSensor(coordinator, runtime),
            AquilaClockDriftSensor(coordinator, runtime),
            AquilaLastCsiPacketSensor(coordinator, runtime),
        ]
    )

    known: set[str] = set()

    @callback
    def _discover() -> None:
        new_entities: list[SensorEntity] = []
        zones = (coordinator.data or {}).get("zones", {}) or {}

        home = _find_home_zone(zones)
        if home is not None and METRIC_PERSON_COUNT not in known:
            known.add(METRIC_PERSON_COUNT)
            new_entities.append(
                AquilaPersonCountSensor(
                    coordinator, runtime, home[0], core_device_info(runtime)
                )
            )

        for zone_id, zone in zones.items():
            kind = zone.get("kind")
            is_home = kind == ZONE_KIND_HOME
            is_room = kind == ZONE_KIND_ROOM
            if kind == ZONE_KIND_FLOOR:
                continue  # vitals are room and home scoped only
            if is_room and not runtime.expose_room_vitals:
                continue
            if not (is_home or is_room):
                continue
            device_info = (
                core_device_info(runtime)
                if is_home
                else zone_device_info(runtime, zone_id, zone)
            )
            for metric in _VITAL_METRICS:
                key = f"{metric}:{zone_id}"
                if key in known:
                    continue
                known.add(key)
                new_entities.append(
                    AquilaVitalsSensor(
                        coordinator, runtime, zone_id, metric, device_info
                    )
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


class AquilaPersonCountSensor(AquilaEntity, SensorEntity):
    """Home-level person count (integer, v1)."""

    _attr_translation_key = METRIC_PERSON_COUNT
    _attr_icon = "mdi:account-multiple"
    _attr_native_unit_of_measurement = "people"

    def __init__(
        self,
        coordinator: AquilaCoordinator,
        runtime: AquilaRuntimeData,
        zone_id: str,
        device_info: DeviceInfo,
    ) -> None:
        super().__init__(
            coordinator,
            runtime,
            unique_id=f"aquila:{runtime.instance_id}:{zone_id}:{METRIC_PERSON_COUNT}",
            device_info=device_info,
        )
        self._zone_id = zone_id

    @property
    def native_value(self) -> int | None:
        zone = (self.coordinator.data or {}).get("zones", {}).get(self._zone_id, {})
        value = zone.get("person_count")
        return None if value is None else int(value)


class AquilaVitalsSensor(AquilaEntity, SensorEntity):
    """Per-zone (room/home) breathing or heart rate."""

    _attr_device_class = SensorDeviceClass.MEASUREMENT
    _attr_state_class = SensorStateClass.MEASUREMENT

    def __init__(
        self,
        coordinator: AquilaCoordinator,
        runtime: AquilaRuntimeData,
        zone_id: str,
        metric: str,
        device_info: DeviceInfo,
    ) -> None:
        super().__init__(
            coordinator,
            runtime,
            unique_id=f"aquila:{runtime.instance_id}:{zone_id}:{metric}",
            device_info=device_info,
        )
        self._zone_id = zone_id
        self._metric = metric
        self._attr_translation_key = metric
        self._attr_native_unit_of_measurement = _VITAL_METRICS[metric]

    @property
    def native_value(self) -> float | None:
        zone = (self.coordinator.data or {}).get("zones", {}).get(self._zone_id, {})
        value = zone.get(self._metric)
        return None if value is None else float(value)


class AquilaNodesOnlineSensor(AquilaEntity, SensorEntity):
    """Diagnostic: nodes currently online."""

    _attr_entity_category = EntityCategory.DIAGNOSTIC
    _attr_translation_key = METRIC_NODES_ONLINE
    _attr_native_unit_of_measurement = "nodes"

    def __init__(
        self,
        coordinator: AquilaCoordinator,
        runtime: AquilaRuntimeData,
    ) -> None:
        super().__init__(
            coordinator,
            runtime,
            unique_id=f"aquila:{runtime.instance_id}:{SYSTEM_ZONE_ID}:{METRIC_NODES_ONLINE}",
            device_info=core_device_info(runtime),
        )

    @property
    def native_value(self) -> int | None:
        return _as_int(self.health.get(HEALTH_NODES_ONLINE))


class AquilaNodesTotalSensor(AquilaEntity, SensorEntity):
    """Diagnostic: nodes known to the core."""

    _attr_entity_category = EntityCategory.DIAGNOSTIC
    _attr_translation_key = METRIC_NODES_TOTAL
    _attr_native_unit_of_measurement = "nodes"

    def __init__(
        self,
        coordinator: AquilaCoordinator,
        runtime: AquilaRuntimeData,
    ) -> None:
        super().__init__(
            coordinator,
            runtime,
            unique_id=f"aquila:{runtime.instance_id}:{SYSTEM_ZONE_ID}:{METRIC_NODES_TOTAL}",
            device_info=core_device_info(runtime),
        )

    @property
    def native_value(self) -> int | None:
        return _as_int(self.health.get(HEALTH_NODES_TOTAL))


class AquilaCalibrationStateSensor(AquilaEntity, SensorEntity):
    """Diagnostic: calibration state (enum)."""

    _attr_device_class = SensorDeviceClass.ENUM
    _attr_entity_category = EntityCategory.DIAGNOSTIC
    _attr_translation_key = METRIC_CALIBRATION_STATE
    _attr_options = list(CALIBRATION_STATES)

    def __init__(
        self,
        coordinator: AquilaCoordinator,
        runtime: AquilaRuntimeData,
    ) -> None:
        super().__init__(
            coordinator,
            runtime,
            unique_id=f"aquila:{runtime.instance_id}:{SYSTEM_ZONE_ID}:{METRIC_CALIBRATION_STATE}",
            device_info=core_device_info(runtime),
        )

    @property
    def native_value(self) -> str | None:
        value = self.health.get(HEALTH_CALIBRATION_STATE)
        # Tolerate unknown enum values by reporting unknown (home-assistant.md §5).
        return value if value in CALIBRATION_STATES else None


class AquilaClockDriftSensor(AquilaEntity, SensorEntity):
    """Diagnostic: node clock drift estimate, in milliseconds."""

    _attr_device_class = SensorDeviceClass.MEASUREMENT
    _attr_state_class = SensorStateClass.MEASUREMENT
    _attr_entity_category = EntityCategory.DIAGNOSTIC
    _attr_translation_key = METRIC_CLOCK_DRIFT
    _attr_native_unit_of_measurement = "ms"

    def __init__(
        self,
        coordinator: AquilaCoordinator,
        runtime: AquilaRuntimeData,
    ) -> None:
        super().__init__(
            coordinator,
            runtime,
            unique_id=f"aquila:{runtime.instance_id}:{SYSTEM_ZONE_ID}:{METRIC_CLOCK_DRIFT}",
            device_info=core_device_info(runtime),
        )

    @property
    def native_value(self) -> float | None:
        value = self.health.get(HEALTH_CLOCK_DRIFT_MS)
        return None if value is None else float(value)


class AquilaLastCsiPacketSensor(AquilaEntity, SensorEntity):
    """Diagnostic: timestamp of the last CSI packet observed by the core."""

    _attr_device_class = SensorDeviceClass.TIMESTAMP
    _attr_entity_category = EntityCategory.DIAGNOSTIC
    _attr_translation_key = METRIC_LAST_CSI_PACKET

    def __init__(
        self,
        coordinator: AquilaCoordinator,
        runtime: AquilaRuntimeData,
    ) -> None:
        super().__init__(
            coordinator,
            runtime,
            unique_id=f"aquila:{runtime.instance_id}:{SYSTEM_ZONE_ID}:{METRIC_LAST_CSI_PACKET}",
            device_info=core_device_info(runtime),
        )

    @property
    def native_value(self) -> Any:
        raw = self.health.get(HEALTH_LAST_PACKET_TS)
        if not isinstance(raw, str):
            return None
        parsed = dt_util.parse_datetime(raw)
        if parsed is None:
            return None
        if parsed.tzinfo is None:
            parsed = parsed.replace(tzinfo=dt_util.UTC)
        return parsed


def _as_int(value: Any) -> int | None:
    return None if value is None else int(value)
