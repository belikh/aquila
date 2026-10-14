"""Firmware update platform for Aquila: one ``update`` entity per node.

Implements ``docs/spec/firmware-ota.md`` §7 and home-assistant.md §6.1:
``update.aquila_<node>_firmware`` with ``installed_version``,
``latest_version``, ``update_available``, an install action, ``in_progress`` and
``update_percentage``. OTA rides the node control channel; the component only
drives the core's private API.
"""

from __future__ import annotations

import logging
from typing import Any

from homeassistant.components.update import (
    UpdateDeviceClass,
    UpdateEntity,
    UpdateEntityFeature,
)
from homeassistant.config_entries import ConfigEntry
from homeassistant.const import EntityCategory
from homeassistant.core import HomeAssistant, callback
from homeassistant.exceptions import HomeAssistantError
from homeassistant.helpers.entity_platform import AddEntitiesCallback

from .const import DOMAIN, METRIC_FIRMWARE
from .coordinator import (
    AquilaCoordinator,
    AquilaEntity,
    AquilaRuntimeData,
    node_device_info,
)

_LOGGER = logging.getLogger(__name__)


async def async_setup_entry(
    hass: HomeAssistant,
    entry: ConfigEntry,
    async_add_entities: AddEntitiesCallback,
) -> None:
    """Set up one firmware update entity per node."""
    runtime: AquilaRuntimeData = hass.data[DOMAIN][entry.entry_id]
    coordinator = runtime.coordinator
    known: set[str] = set()

    @callback
    def _discover() -> None:
        nodes = (coordinator.data or {}).get("nodes", {}) or {}
        new_entities: list[UpdateEntity] = []
        for mac, node in nodes.items():
            if mac in known or not isinstance(node, dict):
                continue
            known.add(mac)
            new_entities.append(
                AquilaFirmwareUpdateEntity(coordinator, runtime, mac, node)
            )
        if new_entities:
            async_add_entities(new_entities)

    _discover()
    entry.async_on_unload(coordinator.async_add_listener(_discover))


class AquilaFirmwareUpdateEntity(AquilaEntity, UpdateEntity):
    """Per-node firmware update entity."""

    _attr_device_class = UpdateDeviceClass.FIRMWARE
    _attr_entity_category = EntityCategory.DIAGNOSTIC
    _attr_translation_key = METRIC_FIRMWARE
    _attr_supported_features = (
        UpdateEntityFeature.INSTALL | UpdateEntityFeature.PROGRESS
    )

    def __init__(
        self,
        coordinator: AquilaCoordinator,
        runtime: AquilaRuntimeData,
        mac: str,
        node: dict[str, Any],
    ) -> None:
        super().__init__(
            coordinator,
            runtime,
            unique_id=f"aquila:{runtime.instance_id}:node:{mac}:{METRIC_FIRMWARE}",
            device_info=node_device_info(runtime, node),
        )
        self._mac = mac
        self._attr_title = node.get("name") or mac

    @property
    def _firmware(self) -> dict[str, Any]:
        node = (self.coordinator.data or {}).get("nodes", {}).get(self._mac, {})
        return node.get("firmware", {}) if isinstance(node, dict) else {}

    @property
    def installed_version(self) -> str | None:
        value = self._firmware.get("installed_version")
        return str(value) if value is not None else None

    @property
    def latest_version(self) -> str | None:
        # The core owns the latest version per node (firmware-ota.md §7); the
        # component presents it and does not rank images itself
        # (sister-issues-firmware-ha.md F13).
        value = self._firmware.get("latest_version")
        if value is not None:
            return str(value)
        return self.installed_version

    @property
    def in_progress(self) -> bool:
        return bool(self._firmware.get("in_progress"))

    @property
    def update_percentage(self) -> float | None:
        value = self._firmware.get("update_percentage")
        return None if value is None else float(value)

    async def async_install(
        self, version: str | None, backup: bool, **kwargs: Any
    ) -> None:
        """Begin an OTA for this node."""
        image_id = self._firmware.get("image_id") or version
        if not image_id:
            latest = self.latest_version
            image_id = latest
        if not image_id:
            raise HomeAssistantError(
                f"no firmware image available for node {self._mac}"
            )
        await self._runtime.client.async_install_firmware(
            self._mac, str(image_id), force=False
        )
        await self.coordinator.async_request_refresh()

    async def async_abort(self) -> None:
        """Abort an in-progress OTA."""
        await self._runtime.client.async_abort_firmware(self._mac)
        await self.coordinator.async_request_refresh()
