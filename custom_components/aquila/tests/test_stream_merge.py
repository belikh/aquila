"""Tests for the coordinator's firmware snapshot merge (stream.py).

These exercise the pure helpers and ``async_collect_snapshot`` with a stub
client, so no HTTP and no HA runtime are involved.
"""

from __future__ import annotations

import pytest

from custom_components.aquila.stream import (
    async_collect_snapshot,
    firmware_progress_percent,
    manifest_images,
    merge_node_firmware,
    normalise_nodes,
    resolve_image,
)

from helpers import StubSnapshotClient

IMAGES = [
    {"id": "aaa", "version": "1.0.0", "notes": "first"},
    {"id": "ccc", "version": "1.2.0", "notes": "newest"},
    {"id": "bbb", "version": "1.1.0"},
]


def test_normalise_nodes_from_items() -> None:
    payload = {"items": [{"mac": "aa", "name": "A"}, {"id": "bb", "name": "B"}]}
    nodes = normalise_nodes(payload)
    assert set(nodes) == {"aa", "bb"}


def test_manifest_images_tolerates_junk() -> None:
    assert manifest_images(None) == []
    assert manifest_images({"items": "nope"}) == []
    assert manifest_images({"items": IMAGES}) == IMAGES


def test_resolve_image_by_version_and_latest() -> None:
    assert resolve_image(IMAGES, "1.1.0")["id"] == "bbb"
    assert resolve_image(IMAGES, "9.9.9") is None
    assert resolve_image(IMAGES, None)["id"] == "ccc"
    assert resolve_image([], None) is None


def test_progress_percent_from_fraction_and_bytes() -> None:
    assert firmware_progress_percent({"progress": {"fraction": 0.42}}) == 42.0
    assert (
        firmware_progress_percent(
            {"progress": {"receivedBytes": 50, "totalBytes": 200}}
        )
        == 25.0
    )
    assert firmware_progress_percent({"progress": {"fraction": 1.5}}) == 100.0
    assert firmware_progress_percent({}) is None
    assert firmware_progress_percent(None) is None


def test_merge_offers_update_when_latest_differs() -> None:
    merged = merge_node_firmware(
        {"installed_version": "1.0.0", "latest_version": "1.2.0"}, None, IMAGES
    )
    assert merged["installed_version"] == "1.0.0"
    assert merged["latest_version"] == "1.2.0"
    assert merged["update_available"] is True
    assert merged["in_progress"] is False
    assert merged["update_percentage"] is None
    assert merged["release_notes"] == "newest"
    assert merged["image_id"] == "ccc"


def test_merge_no_update_when_versions_match() -> None:
    merged = merge_node_firmware(
        {"installed_version": "1.2.0", "latest_version": "1.2.0"}, None, IMAGES
    )
    assert merged["update_available"] is False


def test_merge_no_offer_when_no_store() -> None:
    merged = merge_node_firmware(
        {"installed_version": "1.0.0", "latest_version": None}, None, []
    )
    assert merged["update_available"] is False
    assert merged["latest_version"] is None
    assert merged["release_notes"] is None


def test_merge_running_job_sets_progress_and_image() -> None:
    merged = merge_node_firmware(
        {"installed_version": "1.0.0", "latest_version": "1.2.0", "pending": True},
        {"state": "running", "imageId": "ccc", "progress": {"fraction": 0.25}},
        IMAGES,
    )
    assert merged["in_progress"] is True
    assert merged["update_percentage"] == 25.0
    assert merged["image_id"] == "ccc"


@pytest.mark.asyncio
async def test_collect_snapshot_merges_per_node_firmware() -> None:
    client = StubSnapshotClient(
        state={"zones": {}, "health": {}},
        nodes={"items": [{"mac": "aa", "name": "A"}, {"mac": "bb", "name": "B"}]},
        manifest={"items": IMAGES},
        firmware={
            "aa": {"installed_version": "1.0.0", "latest_version": "1.2.0"},
            "bb": {"installed_version": "1.1.0", "latest_version": "1.1.0"},
        },
        jobs={"aa": {"state": "running", "progress": {"fraction": 0.5}}},
    )
    snapshot = await async_collect_snapshot(client)

    assert snapshot["health"]["core_connected"] is True
    assert snapshot["firmware_manifest"] == {"items": IMAGES}
    aa = snapshot["nodes"]["aa"]["firmware"]
    assert aa["latest_version"] == "1.2.0"
    assert aa["in_progress"] is True
    assert aa["update_percentage"] == 50.0
    bb = snapshot["nodes"]["bb"]["firmware"]
    assert bb["update_available"] is False


@pytest.mark.asyncio
async def test_collect_snapshot_is_best_effort_per_node() -> None:
    client = StubSnapshotClient(
        state={"zones": {}},
        nodes={
            "items": [
                {"mac": "aa", "firmware": {"installed_version": "1.0.0"}},
                {"mac": "bb", "firmware": {"installed_version": "1.0.0"}},
            ]
        },
        manifest={"items": IMAGES},
        firmware={
            "aa": {"installed_version": "1.0.0", "latest_version": "1.2.0"},
            "bb": {"installed_version": "1.0.0", "latest_version": None},
        },
        fail_firmware_for={"bb"},
    )
    snapshot = await async_collect_snapshot(client)
    assert snapshot["nodes"]["aa"]["firmware"]["latest_version"] == "1.2.0"
    # bb's fetch failed, so its original /nodes firmware block is preserved.
    assert snapshot["nodes"]["bb"]["firmware"]["installed_version"] == "1.0.0"
    assert "latest_version" not in snapshot["nodes"]["bb"]["firmware"]


@pytest.mark.asyncio
async def test_collect_snapshot_survives_nodes_failure() -> None:
    client = StubSnapshotClient(
        state={"zones": {}, "health": {"nodes_online": 1}},
        nodes={"items": []},
        manifest={"items": []},
        fail_nodes=True,
    )
    snapshot = await async_collect_snapshot(client)
    assert snapshot["nodes"] == {}
    assert snapshot["health"]["nodes_online"] == 1
