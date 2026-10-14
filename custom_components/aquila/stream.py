"""HTTP polling transport for the Aquila core.

v1 serves **no WebSocket**: ``GET /api/v1/stream`` returns a typed
``501 websocket_unimplemented`` (core-api.md §10), so the component polls the
canonical REST snapshot endpoints instead (home-assistant.md §5.2/§6.2). This
module owns that polling transport; ``AquilaCoordinator`` schedules it.

It holds no state and does not merge *policy*: it fetches, normalises and merges
the core's firmware resources into the snapshot shape the coordinator caches.
The firmware merge is deliberately a set of pure helpers
(:func:`manifest_images`, :func:`resolve_image`, :func:`merge_node_firmware`) so
it can be tested without a Home Assistant runtime.
"""

from __future__ import annotations

import asyncio
import logging
from typing import Any

from .api import AquilaApiClient, AquilaError, parse_version
from .const import (
    FIRMWARE_IMAGE_ID,
    FIRMWARE_IN_PROGRESS,
    FIRMWARE_INSTALLED_VERSION,
    FIRMWARE_LAST_RESULT,
    FIRMWARE_LATEST_VERSION,
    FIRMWARE_PENDING,
    FIRMWARE_RELEASE_NOTES,
    FIRMWARE_UPDATE_AVAILABLE,
    FIRMWARE_UPDATE_PERCENTAGE,
    HEALTH_CORE_CONNECTED,
    IMAGE_ID,
    IMAGE_NOTES,
    IMAGE_VERSION,
    JOB_IMAGE_ID,
    JOB_IMAGE_ID_SNAKE,
    JOB_PROGRESS,
    JOB_PROGRESS_FRACTION,
    JOB_PROGRESS_RECEIVED,
    JOB_PROGRESS_TOTAL,
    JOB_STATE,
    JOB_STATE_RUNNING,
    MANIFEST_ITEMS,
)

_LOGGER = logging.getLogger(__name__)


def normalise_nodes(payload: Any) -> dict[str, Any]:
    """Normalise a ``GET /api/v1/nodes`` payload into a MAC-keyed mapping.

    The core returns ``{"items": [...]}``; a node is keyed by its MAC (falling
    back to its id). An already MAC-keyed mapping is passed through.
    """
    if isinstance(payload, dict):
        items = payload.get(MANIFEST_ITEMS)
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


def manifest_images(manifest: Any) -> list[dict[str, Any]]:
    """Return the image list from a ``GET /api/v1/firmware`` manifest.

    The core serves ``{"items": [...]}``; anything else yields an empty list
    rather than raising, because the manifest is fetched best-effort.
    """
    if not isinstance(manifest, dict):
        return []
    items = manifest.get(MANIFEST_ITEMS)
    if not isinstance(items, list):
        return []
    return [item for item in items if isinstance(item, dict)]


def resolve_image(
    images: list[dict[str, Any]], version: str | None = None
) -> dict[str, Any] | None:
    """Resolve a stored image by version, or the newest image when ``version`` is None.

    An explicit ``version`` that is not in the store resolves to ``None``; the
    caller must not silently install a different image (firmware-ota.md §7).
    """
    if version is not None:
        wanted = str(version)
        for image in images:
            if str(image.get(IMAGE_VERSION)) == wanted:
                return image
        return None
    if not images:
        return None
    return max(images, key=lambda image: parse_version(image.get(IMAGE_VERSION)))


def firmware_progress_percent(job: Any) -> float | None:
    """Map a ``GET .../firmware/job`` payload to a 0–100 percentage.

    The core serves ``progress.fraction`` (0–1) and byte counters; either is
    accepted. A missing or non-numeric progress yields ``None`` rather than a
    fabricated zero.
    """
    if not isinstance(job, dict):
        return None
    progress = job.get(JOB_PROGRESS)
    if not isinstance(progress, dict):
        return None
    fraction = progress.get(JOB_PROGRESS_FRACTION)
    if fraction is None:
        received = progress.get(JOB_PROGRESS_RECEIVED)
        total = progress.get(JOB_PROGRESS_TOTAL)
        if isinstance(received, (int, float)) and isinstance(total, (int, float)) and total:
            fraction = received / total
    if not isinstance(fraction, (int, float)):
        return None
    return max(0.0, min(100.0, round(float(fraction) * 100.0, 2)))


def merge_node_firmware(
    firmware: Any,
    job: Any = None,
    images: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    """Merge per-node firmware state and its OTA job into the snapshot shape.

    ``installed_version`` and the offered ``latest_version`` come from
    ``GET /api/v1/nodes/{node}/firmware``; the job adds live ``in_progress`` and
    progress. The release notes are the notes of the offered image in the store
    manifest, when one matches. Nothing is fabricated: absent values stay
    ``None``/``False`` (firmware-ota.md §7).
    """
    fw: dict[str, Any] = firmware if isinstance(firmware, dict) else {}
    store = images or []

    installed = fw.get(FIRMWARE_INSTALLED_VERSION)
    latest = fw.get(FIRMWARE_LATEST_VERSION)
    pending = bool(fw.get(FIRMWARE_PENDING))

    job_dict: dict[str, Any] = job if isinstance(job, dict) else {}
    job_state = job_dict.get(JOB_STATE)
    in_progress = pending or job_state == JOB_STATE_RUNNING

    image_id = job_dict.get(JOB_IMAGE_ID) or job_dict.get(JOB_IMAGE_ID_SNAKE)
    notes: str | None = None
    if latest is not None:
        offered = resolve_image(store, latest)
        if offered is not None:
            if image_id is None:
                image_id = offered.get(IMAGE_ID)
            notes = offered.get(IMAGE_NOTES) or None

    percentage = firmware_progress_percent(job_dict) if in_progress else None
    update_available = latest is not None and str(latest) != str(installed)

    return {
        FIRMWARE_INSTALLED_VERSION: installed,
        FIRMWARE_LATEST_VERSION: latest,
        FIRMWARE_IMAGE_ID: image_id,
        FIRMWARE_IN_PROGRESS: in_progress,
        FIRMWARE_UPDATE_PERCENTAGE: percentage,
        FIRMWARE_UPDATE_AVAILABLE: update_available,
        FIRMWARE_RELEASE_NOTES: notes,
        FIRMWARE_PENDING: pending,
        FIRMWARE_LAST_RESULT: fw.get(FIRMWARE_LAST_RESULT),
    }


async def _merge_node_firmware(
    client: AquilaApiClient,
    mac: str,
    node: dict[str, Any],
    images: list[dict[str, Any]],
) -> None:
    """Fetch and merge one node's firmware state; best-effort per node."""
    try:
        firmware = await client.async_get_node_firmware(mac)
    except AquilaError as err:
        _LOGGER.debug("Node firmware fetch failed for %s: %s", mac, err)
        return
    job: Any = None
    try:
        job = await client.async_get_firmware_job(mac)
    except AquilaError as err:
        _LOGGER.debug("Firmware job fetch failed for %s: %s", mac, err)
    node["firmware"] = merge_node_firmware(firmware, job, images)


async def async_collect_snapshot(client: AquilaApiClient) -> dict[str, Any]:
    """Collect the current core snapshot by polling the canonical endpoints.

    ``GET /api/v1/state`` is required; ``GET /api/v1/nodes``,
    ``GET /api/v1/firmware`` and the per-node
    ``GET /api/v1/nodes/{node}/firmware`` (+ ``/job``) are best-effort and never
    fail the poll (home-assistant.md §6.2, firmware-ota.md §7). The state
    response is a plain resource with no stream envelope (core-api.md §10).
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

    nodes: dict[str, Any] = {}
    try:
        nodes = normalise_nodes(await client.async_get_nodes())
        snapshot["nodes"] = nodes
    except AquilaError as err:
        _LOGGER.debug("Node reconciliation failed: %s", err)
        snapshot.setdefault("nodes", {})

    images: list[dict[str, Any]] = []
    try:
        manifest = await client.async_get_firmware_manifest()
        snapshot["firmware_manifest"] = manifest
        images = manifest_images(manifest)
    except AquilaError as err:
        _LOGGER.debug("Firmware manifest reconciliation failed: %s", err)

    if nodes:
        await asyncio.gather(
            *(
                _merge_node_firmware(client, mac, node, images)
                for mac, node in nodes.items()
                if isinstance(node, dict)
            ),
            return_exceptions=True,
        )

    return snapshot
