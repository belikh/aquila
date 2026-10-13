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

from homeassistant.components import frontend, panel_custom
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
    HEALTH_API_VERSION,
    HEALTH_INSTANCE_ID,
    HEALTH_MIN_COMPONENT_VERSION,
    PLATFORMS,
    SERVICE_INSTALL_FIRMWARE,
)
from .coordinator import AquilaCoordinator, AquilaRuntimeData
from .stream import AquilaStreamClient

_LOGGER = logging.getLogger(__name__)

__all__ = ["async_setup", "async_setup_entry", "async_unload_entry", "async_migrate_entry"]

# Reserved keys in ``hass.data[DOMAIN]``; entries are keyed by ``entry_id``.
_PANEL_FLAG = "_panel_registered"
_SERVICES_FLAG = "_services_registered"

SERVICE_INSTALL_FIRMWARE_SCHEMA = vol.Schema(
    {
        vol.Required("node"): cv.string,
        vol.Optional("image_id"): cv.string,
        vol.Optional("version"): cv.string,
        vol.Optional("force", default=False): cv.boolean,
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

    def _on_connected() -> None:
        # A fresh WS connection restarts its sequence; resync the snapshot.
        coordinator.reset_seq()
        coordinator.set_stream_connected(True)
        hass.async_create_task(coordinator.async_request_refresh())

    def _on_disconnected() -> None:
        coordinator.set_stream_connected(False)

    def _on_gap() -> None:
        # A seq gap forces a resync (home-assistant.md §5.2).
        hass.async_create_task(coordinator.async_request_refresh())

    stream = AquilaStreamClient(
        hass,
        client,
        on_event=coordinator.apply_event,
        on_connected=_on_connected,
        on_disconnected=_on_disconnected,
        on_gap=_on_gap,
    )

    runtime = AquilaRuntimeData(
        client=client,
        coordinator=coordinator,
        instance_id=instance_id,
        api_version=str(health.get(HEALTH_API_VERSION) or ""),
        min_component_version=health.get(HEALTH_MIN_COMPONENT_VERSION),
        update_interval=update_interval,
        expose_floors=expose_floors,
        expose_room_vitals=expose_room_vitals,
        stream=stream,
    )
    domain_data[entry.entry_id] = runtime

    await coordinator.async_config_entry_first_refresh()
    await stream.async_start()

    await hass.config_entries.async_forward_entry_setups(entry, PLATFORMS)

    _async_register_panel(hass, instance_id)
    _async_register_services(hass)

    entry.async_on_unload(entry.add_update_listener(_async_update_listener))
    return True


async def async_unload_entry(hass: HomeAssistant, entry: ConfigEntry) -> bool:
    """Unload a config entry, stopping the stream and tearing down shared state."""
    domain_data = hass.data.get(DOMAIN, {})
    runtime: AquilaRuntimeData | None = domain_data.get(entry.entry_id)

    unload_ok = await hass.config_entries.async_unload_platforms(entry, PLATFORMS)
    if not unload_ok:
        return False

    if runtime is not None and runtime.stream is not None:
        await runtime.stream.async_stop()
    domain_data.pop(entry.entry_id, None)

    remaining = [
        other
        for other in hass.config_entries.async_entries(DOMAIN)
        if other.entry_id != entry.entry_id
        and other.state is ConfigEntryState.LOADED
    ]
    if not remaining:
        _remove_panel(hass)
        _async_unregister_services(hass)
    return True


async def _async_update_listener(hass: HomeAssistant, entry: ConfigEntry) -> None:
    """Reload the entry when its options change (home-assistant.md §7)."""
    await hass.config_entries.async_reload(entry.entry_id)


def _async_register_panel(hass: HomeAssistant, instance_id: str) -> None:
    """Register the admin-gated sidebar panel once for the domain (home-assistant.md §8)."""
    domain_data = hass.data.setdefault(DOMAIN, {})
    if domain_data.get(_PANEL_FLAG):
        return
    try:
        panel_custom.async_register_panel(
            hass,
            webcomponent_path="aquila-panel.js",
            frontend_url_path=DOMAIN,
            sidebar_title="Aquila",
            sidebar_icon="mdi:radar",
            config={"instance_id": instance_id},
            admin_only=True,
            require_admin=True,
        )
    except Exception as err:  # noqa: BLE001 - panel registration is best-effort
        _LOGGER.warning("Could not register the Aquila panel: %s", err)
        return
    domain_data[_PANEL_FLAG] = True


def _remove_panel(hass: HomeAssistant) -> None:
    """Remove the component panel when the last entry unloads."""
    domain_data = hass.data.get(DOMAIN, {})
    if not domain_data.get(_PANEL_FLAG):
        return
    try:
        frontend.async_remove_panel(hass, DOMAIN)
    except Exception as err:  # noqa: BLE001 - teardown must not fail the unload
        _LOGGER.debug("Could not remove the Aquila panel: %s", err)
    domain_data[_PANEL_FLAG] = False


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
    node = call.data["node"]
    image_id: str | None = call.data.get("image_id") or call.data.get("version")
    force = bool(call.data.get("force", False))

    for _entry_id, runtime in _iter_runtimes(hass):
        nodes = (runtime.coordinator.data or {}).get("nodes", {}) or {}
        if node not in nodes:
            continue
        if not image_id:
            raise HomeAssistantError(
                f"No image_id or version supplied for node {node}"
            )
        await runtime.client.async_install_firmware(node, str(image_id), force=force)
        await runtime.coordinator.async_request_refresh()
        return

    raise HomeAssistantError(f"Unknown Aquila node: {node}")


def _iter_runtimes(hass: HomeAssistant) -> Iterator[tuple[str, AquilaRuntimeData]]:
    for key, value in hass.data.get(DOMAIN, {}).items():
        if isinstance(value, AquilaRuntimeData):
            yield key, value
