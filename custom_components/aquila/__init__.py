"""The Aquila HACS custom component.

Presentation and transport only: the component owns no DSP, inference, fusion or
calibration and speaks only to the core's private HTTP+WS API. Setup is async,
raises ``ConfigEntryNotReady`` when the core is unreachable and ``ConfigEntryError``
on a version mismatch (home-assistant.md §6.2/§9).
"""

from __future__ import annotations

import logging
from collections.abc import Iterator

import voluptuous as vol

from homeassistant.config_entries import ConfigEntry, ConfigEntryState
from homeassistant.core import HomeAssistant, ServiceCall
from homeassistant.exceptions import (
    ConfigEntryError,
    ConfigEntryNotReady,
    HomeAssistantError,
)
from homeassistant.helpers import config_validation as cv
from homeassistant.helpers.aiohttp_client import async_get_clientsession
from homeassistant.helpers.typing import ConfigType

from .api import (
    AquilaApiClient,
    AquilaAuthError,
    AquilaConnectionError,
    AquilaError,
    AquilaVersionError,
)
from .config_flow import async_migrate_entry
from .const import (
    CONF_EXPOSE_FLOORS,
    CONF_EXPOSE_ROOM_VITALS,
    CONF_TOKEN,
    CONF_UPDATE_INTERVAL,
    CONF_URL,
    DEFAULT_EXPOSE_FLOORS,
    DEFAULT_EXPOSE_ROOM_VITALS,
    DEFAULT_UPDATE_INTERVAL,
    DOMAIN,
    FIRMWARE_IMAGE_ID,
    HEALTH_API_VERSION,
    HEALTH_INSTANCE_ID,
    HEALTH_MIN_COMPONENT_VERSION,
    IMAGE_ID,
    PLATFORMS,
    SERVICE_FIELD_FORCE,
    SERVICE_FIELD_IMAGE_ID,
    SERVICE_FIELD_NODE,
    SERVICE_FIELD_VERSION,
    SERVICE_INSTALL_FIRMWARE,
)
from .coordinator import AquilaCoordinator, AquilaRuntimeData
from .stream import manifest_images, resolve_image

_LOGGER = logging.getLogger(__name__)

__all__ = ["async_setup", "async_setup_entry", "async_unload_entry", "async_migrate_entry"]

# Reserved keys in ``hass.data[DOMAIN]``; entries are keyed by ``entry_id``.
_SERVICES_FLAG = "_services_registered"

SERVICE_INSTALL_FIRMWARE_SCHEMA = vol.Schema(
    {
        vol.Required(SERVICE_FIELD_NODE): cv.string,
        vol.Optional(SERVICE_FIELD_IMAGE_ID): cv.string,
        vol.Optional(SERVICE_FIELD_VERSION): cv.string,
        vol.Optional(SERVICE_FIELD_FORCE, default=False): cv.boolean,
    }
)


async def async_setup(hass: HomeAssistant, config: ConfigType) -> bool:
    """Set up the Aquila integration (no YAML configuration)."""
    hass.data.setdefault(DOMAIN, {})
    return True


async def async_setup_entry(hass: HomeAssistant, entry: ConfigEntry) -> bool:
    """Set up Aquila from a config entry."""
    domain_data = hass.data.setdefault(DOMAIN, {})
    session = async_get_clientsession(hass)
    url = entry.data[CONF_URL]
    token = entry.data.get(CONF_TOKEN)
    client = AquilaApiClient(session, url, token)

    try:
        health = await client.async_validate()
    except AquilaConnectionError as err:
        raise ConfigEntryNotReady(
            f"Cannot reach the Aquila core at {url}: {err}"
        ) from err
    except AquilaVersionError as err:
        # Retrying will not help a version mismatch; surface it clearly.
        raise ConfigEntryError(str(err)) from err
    except AquilaAuthError as err:
        raise ConfigEntryError(f"The Aquila core rejected the token: {err}") from err
    except AquilaError as err:
        raise ConfigEntryNotReady(
            f"Unexpected response from the Aquila core: {err}"
        ) from err

    instance_id = str(health[HEALTH_INSTANCE_ID])

    options = entry.options
    update_interval = int(
        options.get(CONF_UPDATE_INTERVAL, DEFAULT_UPDATE_INTERVAL)
    )
    expose_floors = bool(options.get(CONF_EXPOSE_FLOORS, DEFAULT_EXPOSE_FLOORS))
    expose_room_vitals = bool(
        options.get(CONF_EXPOSE_ROOM_VITALS, DEFAULT_EXPOSE_ROOM_VITALS)
    )

    coordinator = AquilaCoordinator(hass, client, update_interval, instance_id, entry)

    runtime = AquilaRuntimeData(
        client=client,
        coordinator=coordinator,
        instance_id=instance_id,
        api_version=str(health.get(HEALTH_API_VERSION) or ""),
        min_component_version=health.get(HEALTH_MIN_COMPONENT_VERSION),
        update_interval=update_interval,
        expose_floors=expose_floors,
        expose_room_vitals=expose_room_vitals,
    )
    domain_data[entry.entry_id] = runtime

    # v1 has no WebSocket stream (core-api.md §10); the first refresh is the
    # initial load and the coordinator's interval is the ongoing source.
    await coordinator.async_config_entry_first_refresh()

    await hass.config_entries.async_forward_entry_setups(entry, PLATFORMS)

    _async_register_services(hass)

    entry.async_on_unload(entry.add_update_listener(_async_update_listener))
    return True


async def async_unload_entry(hass: HomeAssistant, entry: ConfigEntry) -> bool:
    """Unload a config entry and tear down shared state."""
    domain_data = hass.data.get(DOMAIN, {})

    unload_ok = await hass.config_entries.async_unload_platforms(entry, PLATFORMS)
    if not unload_ok:
        return False

    domain_data.pop(entry.entry_id, None)

    remaining = [
        other
        for other in hass.config_entries.async_entries(DOMAIN)
        if other.entry_id != entry.entry_id
        and other.state is ConfigEntryState.LOADED
    ]
    if not remaining:
        _async_unregister_services(hass)
    return True


async def _async_update_listener(hass: HomeAssistant, entry: ConfigEntry) -> None:
    """Reload the entry when its options change (home-assistant.md §7)."""
    await hass.config_entries.async_reload(entry.entry_id)


def _async_register_services(hass: HomeAssistant) -> None:
    """Register the Aquila services once (firmware-ota.md §7)."""
    domain_data = hass.data.setdefault(DOMAIN, {})
    if domain_data.get(_SERVICES_FLAG):
        return
    hass.services.async_register(
        DOMAIN,
        SERVICE_INSTALL_FIRMWARE,
        _async_handle_install_firmware,
        schema=SERVICE_INSTALL_FIRMWARE_SCHEMA,
    )
    domain_data[_SERVICES_FLAG] = True


def _async_unregister_services(hass: HomeAssistant) -> None:
    """Remove the Aquila services when the last entry unloads."""
    domain_data = hass.data.get(DOMAIN, {})
    if not domain_data.get(_SERVICES_FLAG):
        return
    hass.services.async_remove(DOMAIN, SERVICE_INSTALL_FIRMWARE)
    domain_data[_SERVICES_FLAG] = False


async def _async_handle_install_firmware(call: ServiceCall) -> None:
    """Handle ``aquila.install_firmware`` (firmware-ota.md §7)."""
    hass = call.hass
    node = call.data[SERVICE_FIELD_NODE]
    image_id: str | None = call.data.get(
        SERVICE_FIELD_IMAGE_ID
    ) or call.data.get(SERVICE_FIELD_VERSION)
    force = bool(call.data.get(SERVICE_FIELD_FORCE, False))

    for _entry_id, runtime in _iter_runtimes(hass):
        nodes = (runtime.coordinator.data or {}).get("nodes", {}) or {}
        if node not in nodes:
            continue
        resolved = image_id or _default_image_id(runtime, node)
        if not resolved:
            raise HomeAssistantError(
                f"No firmware image available for node {node}; "
                "supply image_id or version"
            )
        await runtime.client.async_install_firmware(node, str(resolved), force=force)
        await runtime.coordinator.async_request_refresh()
        return

    raise HomeAssistantError(f"Unknown Aquila node: {node}")


def _default_image_id(runtime: AquilaRuntimeData, node: str) -> str | None:
    """Resolve the newest stored image id for ``node`` from the snapshot.

    The manifest is the source of truth (firmware-ota.md §7); the core ranks
    images, the component only looks one up by version.
    """
    manifest = (runtime.coordinator.data or {}).get("firmware_manifest")
    images = manifest_images(manifest)
    offered = resolve_image(images, None)
    if offered is not None and offered.get(IMAGE_ID):
        return str(offered[IMAGE_ID])
    node_firmware = (
        (runtime.coordinator.data or {}).get("nodes", {}).get(node, {}) or {}
    ).get("firmware", {})
    if isinstance(node_firmware, dict):
        image = node_firmware.get(FIRMWARE_IMAGE_ID)
        if image:
            return str(image)
    return None


def _iter_runtimes(hass: HomeAssistant) -> Iterator[tuple[str, AquilaRuntimeData]]:
    for key, value in hass.data.get(DOMAIN, {}).items():
        if isinstance(value, AquilaRuntimeData):
            yield key, value
