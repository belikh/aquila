"""Tests for the Aquila firmware update entity (update.py).

The real ``CoordinatorEntity``/``UpdateEntity`` classes are used; only the
coordinator, runtime client and snapshot are stubbed, so these run under plain
``pytest`` with no Home Assistant instance.
"""

from __future__ import annotations

import pytest
from homeassistant.components.update import UpdateDeviceClass, UpdateEntity
from homeassistant.exceptions import HomeAssistantError
from homeassistant.helpers.update_coordinator import CoordinatorEntity

from custom_components.aquila.api import AquilaApiError
from custom_components.aquila.stream import merge_node_firmware
from custom_components.aquila.update import (
    AquilaFirmwareUpdateEntity,
    async_setup_entry,
)

from helpers import make_entity

NODE = "aa:bb:cc:dd:ee:ff"
IMAGES = [
    {"id": "aaa", "version": "1.0.0"},
    {"id": "ccc", "version": "1.2.0", "notes": "release notes for 1.2.0"},
]


def _fw(**overrides: object) -> dict[str, object]:
    base = {
        "installed_version": "1.0.0",
        "latest_version": "1.2.0",
        "update_available": True,
        "in_progress": False,
        "update_percentage": None,
        "image_id": "ccc",
        "release_notes": None,
    }
    base.update(overrides)
    return base


def test_entity_is_coordinator_update_entity() -> None:
    entity, _, _ = make_entity(_fw())
    assert isinstance(entity, CoordinatorEntity)
    assert isinstance(entity, UpdateEntity)
    assert entity.device_class == UpdateDeviceClass.FIRMWARE
    assert entity.unique_id == f"aquila:instance-1:node:{NODE}:firmware"


def test_state_mapping_installed_latest_and_available() -> None:
    entity, _, _ = make_entity(_fw())
    assert entity.installed_version == "1.0.0"
    assert entity.latest_version == "1.2.0"
    assert entity.update_available is True
    assert entity.in_progress is False
    assert entity.update_percentage is None
    # HA derives the on/off state from installed vs latest (on = update offered).
    assert entity.state == "on"


def test_no_update_when_versions_match() -> None:
    entity, _, _ = make_entity(
        _fw(latest_version="1.0.0", update_available=False, image_id=None)
    )
    assert entity.update_available is False
    assert entity.state == "off"


def test_latest_falls_back_to_installed_without_store() -> None:
    entity, _, _ = make_entity(
        {"installed_version": "1.0.0", "latest_version": None}
    )
    assert entity.latest_version == "1.0.0"
    assert entity.update_available is False


def test_update_percentage_clamped_and_defensive() -> None:
    entity, _, _ = make_entity(_fw(in_progress=True, update_percentage=42.5))
    assert entity.update_percentage == 42.5
    assert entity.in_progress is True

    entity, _, _ = make_entity(_fw(in_progress=True, update_percentage=250))
    assert entity.update_percentage == 100.0

    entity, _, _ = make_entity(_fw(update_percentage="not-a-number"))
    assert entity.update_percentage is None


def test_release_summary_is_truncated() -> None:
    entity, _, _ = make_entity(_fw(release_notes="x" * 400))
    assert entity.release_summary is not None
    assert len(entity.release_summary) == 255


@pytest.mark.asyncio
async def test_async_release_notes_returns_snapshot_notes() -> None:
    entity, _, _ = make_entity(_fw(release_notes="changelog"))
    assert await entity.async_release_notes() == "changelog"

    entity, _, _ = make_entity(_fw(release_notes=None))
    assert await entity.async_release_notes() is None


@pytest.mark.asyncio
async def test_async_install_uses_snapshot_image_id() -> None:
    entity, coordinator, client = make_entity(_fw(image_id="ccc"))
    await entity.async_install(None, False)
    assert client.install_calls == [(NODE, "ccc", False)]
    assert coordinator.refresh_calls == 1


@pytest.mark.asyncio
async def test_async_install_resolves_version_via_manifest() -> None:
    entity, _, client = make_entity(_fw(image_id=None), manifest={"items": IMAGES})
    await entity.async_install("1.2.0", False)
    assert client.install_calls == [(NODE, "ccc", False)]


@pytest.mark.asyncio
async def test_async_install_defaults_to_latest_image() -> None:
    entity, _, client = make_entity(
        _fw(image_id=None, latest_version="1.2.0"),
        manifest={"items": IMAGES},
    )
    await entity.async_install(None, False)
    assert client.install_calls == [(NODE, "ccc", False)]


@pytest.mark.asyncio
async def test_async_install_without_any_image_raises() -> None:
    entity, _, client = make_entity({}, manifest={"items": []})
    with pytest.raises(HomeAssistantError):
        await entity.async_install(None, False)
    assert client.install_calls == []


@pytest.mark.asyncio
async def test_async_install_surfaces_core_error() -> None:
    entity, _, _ = make_entity(
        _fw(image_id="ccc"),
        fail=AquilaApiError(503, "ota_unavailable"),
    )
    with pytest.raises(HomeAssistantError):
        await entity.async_install(None, False)


@pytest.mark.asyncio
async def test_async_abort_calls_core() -> None:
    entity, coordinator, client = make_entity(_fw(in_progress=True))
    await entity.async_abort()
    assert client.abort_calls == [NODE]
    assert coordinator.refresh_calls == 1


def test_merge_then_entity_reads_merged_snapshot() -> None:
    # The coordinator merge and the entity agree on the firmware snapshot shape.
    merged = merge_node_firmware(
        {"installed_version": "1.0.0", "latest_version": "1.2.0", "pending": True},
        {"state": "running", "progress": {"fraction": 0.75}},
        IMAGES,
    )
    entity, _, _ = make_entity(merged)
    assert entity.installed_version == "1.0.0"
    assert entity.latest_version == "1.2.0"
    assert entity.in_progress is True
    assert entity.update_percentage == 75.0
    assert entity.update_available is True


def test_async_setup_entry_is_importable() -> None:
    # The platform entry point stays importable and callable-shaped.
    assert callable(async_setup_entry)
