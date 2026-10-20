"""Diagnostics support for Aquila (config-entry diagnostics, redacted).

Dumps the coordinator snapshot and the config entry with node MACs and any API
token redacted (home-assistant.md §5.2/§6.1).
"""

from __future__ import annotations

from typing import Any

from homeassistant.components.diagnostics import async_redact_data
from homeassistant.config_entries import ConfigEntry
from homeassistant.core import HomeAssistant

from .const import CONF_TOKEN, DOMAIN
from .coordinator import AquilaRuntimeData

#: Values redacted wherever the key appears. Node MACs are removed separately
#: because they also appear as mapping keys.
TO_REDACT = {CONF_TOKEN, "mac", "mac_address"}


async def async_get_config_entry_diagnostics(
    hass: HomeAssistant, entry: ConfigEntry
) -> dict[str, Any]:
    """Return redacted diagnostics for a config entry."""
    runtime: AquilaRuntimeData = hass.data[DOMAIN][entry.entry_id]
    data = runtime.coordinator.data or {}

    nodes: list[dict[str, Any]] = []
    for node in (data.get("nodes") or {}).values():
        if isinstance(node, dict):
            nodes.append(async_redact_data(node, TO_REDACT))

    return {
        "entry": {
            "data": async_redact_data(dict(entry.data), TO_REDACT),
            "options": dict(entry.options),
            "unique_id": entry.unique_id,
            "version": entry.version,
        },
        "runtime": {
            "instance_id": runtime.instance_id,
            "api_version": runtime.api_version,
            "min_component_version": runtime.min_component_version,
            "update_interval": runtime.update_interval,
            "last_update_success": runtime.coordinator.last_update_success,
        },
        "snapshot": {
            "zones": data.get("zones", {}),
            "health": data.get("health", {}),
            "nodes": nodes,
            "firmware_manifest": data.get("firmware_manifest"),
        },
    }
