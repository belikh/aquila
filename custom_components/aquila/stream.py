"""HTTP polling transport for the Aquila core.

v1 serves **no WebSocket**: ``GET /api/v1/stream`` returns a typed
``501 websocket_unimplemented`` (core-api.md §10), so the component polls the
canonical REST snapshot endpoints instead (home-assistant.md §5.2/§6.2). This
module owns that polling transport; ``AquilaCoordinator`` schedules it.

It holds no state and does not merge: it fetches, normalises and returns the
snapshot the coordinator caches.
"""

from __future__ import annotations

import logging
from typing import Any

from .api import AquilaApiClient, AquilaError
from .const import HEALTH_CORE_CONNECTED

_LOGGER = logging.getLogger(__name__)


def normalise_nodes(payload: Any) -> dict[str, Any]:
    """Normalise a ``GET /api/v1/nodes`` payload into a MAC-keyed mapping.

    The core returns ``{"items": [...]}``; a node is keyed by its MAC (falling
    back to its id). An already MAC-keyed mapping is passed through.
    """
    if isinstance(payload, dict):
        items = payload.get("items")
        if isinstance(items, list):
            raw_nodes = items
        else:
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


async def async_collect_snapshot(client: AquilaApiClient) -> dict[str, Any]:
    """Collect the current core snapshot by polling the canonical endpoints.

    ``GET /api/v1/state`` is required; ``GET /api/v1/nodes`` and
    ``GET /api/v1/firmware`` are best-effort and never fail the poll
    (home-assistant.md §6.2, firmware-ota.md §7). The state response is a plain
    resource with no stream envelope (core-api.md §10).
    """
    state = await client.async_get_state()
    snapshot: dict[str, Any] = dict(state) if isinstance(state, dict) else {}
    snapshot.setdefault("zones", {})
    health = snapshot.get("health")
    if not isinstance(health, dict):
        health = {}
    # A successful poll is the v1 connectivity signal; a failed poll raises
    # before reaching here and the coordinator marks entities unavailable.
    health.setdefault(HEALTH_CORE_CONNECTED, True)
    snapshot["health"] = health

    try:
        snapshot["nodes"] = normalise_nodes(await client.async_get_nodes())
    except AquilaError as err:
        _LOGGER.debug("Node reconciliation failed: %s", err)

    try:
        snapshot["firmware_manifest"] = await client.async_get_firmware_manifest()
    except AquilaError as err:
        _LOGGER.debug("Firmware manifest reconciliation failed: %s", err)

    return snapshot
