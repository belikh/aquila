"""Firmware update platform for Aquila: one ``update`` entity per node.

Implements ``docs/spec/firmware-ota.md`` §7 and home-assistant.md §6.1:
``update.aquila_<node>_firmware`` with ``installed_version``,
``latest_version``, ``update_available``, an install action, ``in_progress`` and
``update_percentage``. OTA rides the node control channel; the component only
drives the core's private API.

The entity is a :class:`~homeassistant.helpers.update_coordinator.CoordinatorEntity`
subclass (via :class:`AquilaEntity`); it reads the firmware state the coordinator
merged from ``GET /api/v1/nodes/{node}/firmware`` and
``GET /api/v1/nodes/{node}/firmware/job`` and never holds its own transport.
``async_install`` calls ``POST /api/v1/nodes/{node}/firmware/install``.

The core OTA subsystem landed with ticket #13; the node-side agent (ticket #14)
and this entity (ticket #15) consume it. On a core with no signed image store or
node transport wired, the install endpoint fails closed with a typed
``503 ota_unavailable``, which surfaces as :class:`HomeAssistantError`.
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

from .api import AquilaApiError, AquilaError
from .const import (
    DOMAIN,
    FIRMWARE_IMAGE_ID,
    FIRMWARE_IN_PROGRESS,
    FIRMWARE_INSTALLED_VERSION,
    FIRMWARE_LATEST_VERSION,
    FIRMWARE_RELEASE_NOTES,
    FIRMWARE_UPDATE_AVAILABLE,
    FIRMWARE_UPDATE_PERCENTAGE,
    IMAGE_ID,
    METRIC_FIRMWARE,
)
from .coordinator import (
    AquilaCoordinator,
    AquilaEntity,
    AquilaRuntimeData,
    node_device_info,
)
from .stream import manifest_images, resolve_image

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
    """Per-node firmware update entity (firmware-ota.md §7)."""

    _attr_device_class = UpdateDeviceClass.FIRMWARE
    _attr_entity_category = EntityCategory.DIAGNOSTIC
    _attr_translation_key = METRIC_FIRMWARE
    _attr_supported_features = (
        UpdateEntityFeature.INSTALL
        | UpdateEntityFeature.PROGRESS
        | UpdateEntityFeature.RELEASE_NOTES
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
        """The merged firmware block for this node, or an empty mapping."""
        nodes = (self.coordinator.data or {}).get("nodes", {}) or {}
        node = nodes.get(self._mac, {})
        if not isinstance(node, dict):
            return {}
        firmware = node.get("firmware", {})
        return firmware if isinstance(firmware, dict) else {}

    @property
    def installed_version(self) -> str | None:
        value = self._firmware.get(FIRMWARE_INSTALLED_VERSION)
        return None if value is None else str(value)

    @property
    def latest_version(self) -> str | None:
        # The core owns the offered version per node (firmware-ota.md §7); the
        # component presents it and does not rank images itself
        # (sister-issues-firmware-ha.md F13). With no store image offered, report
        # the installed version so HA shows the entity as up to date rather than
        # dangling an unknown latest version.
        value = self._firmware.get(FIRMWARE_LATEST_VERSION)
        if value is not None:
            return str(value)
        return self.installed_version

    @property
    def update_available(self) -> bool:
        """Whether the core offers a version different from the installed one."""
        explicit = self._firmware.get(FIRMWARE_UPDATE_AVAILABLE)
        if explicit is not None:
            return bool(explicit)
        latest, installed = self.latest_version, self.installed_version
        return latest is not None and installed is not None and latest != installed

    @property
    def in_progress(self) -> bool:
        return bool(self._firmware.get(FIRMWARE_IN_PROGRESS))

    @property
    def update_percentage(self) -> float | None:
        """Progress as 0–100 (HA's unit); clamped and defensive."""
        value = self._firmware.get(FIRMWARE_UPDATE_PERCENTAGE)
        if value is None:
            return None
        try:
            percent = float(value)
        except (TypeError, ValueError):
            return None
        return max(0.0, min(100.0, percent))

    @property
    def release_summary(self) -> str | None:
        """Short excerpt of the offered image's notes (max 255 chars)."""
        notes = self._firmware.get(FIRMWARE_RELEASE_NOTES)
        if not isinstance(notes, str) or not notes:
            return None
        return notes[:255]

    async def async_install(
        self, version: str | None, backup: bool, **kwargs: Any
    ) -> None:
        """Begin an OTA for this node (``POST .../firmware/install``)."""
        image_id = self._resolve_image_id(version)
        if not image_id:
            raise HomeAssistantError(
                f"no firmware image available for node {self._mac}"
            )
        try:
            await self._runtime.client.async_install_firmware(
                self._mac, image_id, force=False
            )
        except AquilaApiError as err:
            raise HomeAssistantError(
                f"the Aquila core refused the firmware install for "
                f"{self._mac}: {err}"
            ) from err
        except AquilaError as err:
            raise HomeAssistantError(str(err)) from err
        await self.coordinator.async_request_refresh()

    def _resolve_image_id(self, version: str | None) -> str | None:
        """Resolve the store image id for ``version`` (or the offered image)."""
        firmware = self._firmware
        images = manifest_images(
            (self.coordinator.data or {}).get("firmware_manifest")
        )
        if version is None:
            existing = firmware.get(FIRMWARE_IMAGE_ID)
            if existing:
                return str(existing)
            latest = self.latest_version
            offered = resolve_image(images, latest) if latest is not None else None
            if offered is not None and offered.get(IMAGE_ID):
                return str(offered[IMAGE_ID])
            return latest
        matched = resolve_image(images, version)
        if matched is not None and matched.get(IMAGE_ID):
            return str(matched[IMAGE_ID])
        # No store match: hand the version to the core, which validates it.
        return str(version)

    async def async_release_notes(self) -> str | None:
        """Full notes for the offered image, when the store carries them."""
        notes = self._firmware.get(FIRMWARE_RELEASE_NOTES)
        return notes if isinstance(notes, str) and notes else None

    async def async_abort(self) -> None:
        """Abort an in-progress OTA (``POST .../firmware/abort``)."""
        try:
            await self._runtime.client.async_abort_firmware(self._mac)
        except AquilaApiError as err:
            raise HomeAssistantError(
                f"the Aquila core refused to abort the firmware update for "
                f"{self._mac}: {err}"
            ) from err
        except AquilaError as err:
            raise HomeAssistantError(str(err)) from err
        await self.coordinator.async_request_refresh()
