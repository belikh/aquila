"""Tests for the firmware calls on the Aquila REST client.

The client speaks the canonical ``/firmware*`` routes (core-api.md §6,
firmware-ota.md §7). ``aiohttp`` is mocked with :class:`helpers.FakeSession`.
"""

from __future__ import annotations

import aiohttp
import pytest

from custom_components.aquila.api import (
    AquilaApiClient,
    AquilaApiError,
    AquilaAuthError,
    AquilaConnectionError,
)

from helpers import FakeResponse, FakeSession

BASE = "http://core:8099"
NODE = "aa:bb:cc:dd:ee:ff"


def _client(session: FakeSession) -> AquilaApiClient:
    return AquilaApiClient(session, BASE)


@pytest.mark.asyncio
async def test_get_firmware_manifest_hits_canonical_route() -> None:
    session = FakeSession().add(
        "GET", f"{BASE}/api/v1/firmware", FakeResponse(200, {"items": []})
    )
    result = await _client(session).async_get_firmware_manifest()
    assert result == {"items": []}
    assert session.calls[-1].method == "GET"
    assert session.calls[-1].url == f"{BASE}/api/v1/firmware"


@pytest.mark.asyncio
async def test_get_node_firmware_hits_per_node_route() -> None:
    payload = {"installed_version": "1.0.0", "latest_version": "1.1.0"}
    session = FakeSession().add(
        "GET", f"{BASE}/api/v1/nodes/{NODE}/firmware", FakeResponse(200, payload)
    )
    result = await _client(session).async_get_node_firmware(NODE)
    assert result == payload
    assert session.calls[-1].url == f"{BASE}/api/v1/nodes/{NODE}/firmware"


@pytest.mark.asyncio
async def test_install_firmware_posts_image_id_and_force() -> None:
    session = FakeSession().add(
        "POST",
        f"{BASE}/api/v1/nodes/{NODE}/firmware/install",
        FakeResponse(202, {"jobId": "fw-1", "state": "running"}),
    )
    result = await _client(session).async_install_firmware(
        NODE, "abc123", force=True
    )
    assert result["jobId"] == "fw-1"
    call = session.calls[-1]
    assert call.method == "POST"
    assert call.url == f"{BASE}/api/v1/nodes/{NODE}/firmware/install"
    assert call.json == {"image_id": "abc123", "force": True}


@pytest.mark.asyncio
async def test_install_firmware_defaults_force_false() -> None:
    session = FakeSession().add(
        "POST",
        f"{BASE}/api/v1/nodes/{NODE}/firmware/install",
        FakeResponse(202, {"jobId": "fw-2"}),
    )
    await _client(session).async_install_firmware(NODE, "abc123")
    assert session.calls[-1].json == {"image_id": "abc123", "force": False}


@pytest.mark.asyncio
async def test_get_firmware_job_hits_canonical_route() -> None:
    payload = {"state": "running", "progress": {"fraction": 0.5}}
    session = FakeSession().add(
        "GET",
        f"{BASE}/api/v1/nodes/{NODE}/firmware/job",
        FakeResponse(200, payload),
    )
    result = await _client(session).async_get_firmware_job(NODE)
    assert result == payload


@pytest.mark.asyncio
async def test_abort_firmware_posts_to_canonical_route() -> None:
    session = FakeSession().add(
        "POST",
        f"{BASE}/api/v1/nodes/{NODE}/firmware/abort",
        FakeResponse(200, {"state": "aborted"}),
    )
    result = await _client(session).async_abort_firmware(NODE)
    assert result == {"state": "aborted"}
    assert session.calls[-1].method == "POST"


@pytest.mark.asyncio
async def test_ota_unavailable_503_maps_to_api_error() -> None:
    session = FakeSession().add(
        "POST",
        f"{BASE}/api/v1/nodes/{NODE}/firmware/install",
        FakeResponse(503, text="ota_unavailable"),
    )
    with pytest.raises(AquilaApiError) as excinfo:
        await _client(session).async_install_firmware(NODE, "abc123")
    assert excinfo.value.status == 503
    assert "ota_unavailable" in excinfo.value.message


@pytest.mark.asyncio
async def test_401_maps_to_auth_error() -> None:
    session = FakeSession().add(
        "GET", f"{BASE}/api/v1/state", FakeResponse(401)
    )
    with pytest.raises(AquilaAuthError):
        await _client(session).async_get_state()


@pytest.mark.asyncio
async def test_client_error_maps_to_connection_error() -> None:
    session = FakeSession().add(
        "GET",
        f"{BASE}/api/v1/firmware",
        aiohttp.ClientConnectionError("boom"),
    )
    with pytest.raises(AquilaConnectionError):
        await _client(session).async_get_firmware_manifest()
