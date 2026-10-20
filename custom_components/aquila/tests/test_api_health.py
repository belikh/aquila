"""Tests for the health/version gate on the Aquila REST client.

``core-api.md`` §4.1 is the authoritative health schema: the API version lives
in the ``api`` object (``{major, minor}``) and the instance id in
``instance_id``. The flat ``api_version`` key is a tolerated alias
(``home-assistant.md`` §5.2). ``aiohttp`` is mocked with
:class:`helpers.FakeSession`.
"""

from __future__ import annotations

import pytest

from custom_components.aquila.api import (
    AquilaApiClient,
    AquilaError,
    AquilaVersionError,
    _api_major,
    api_version_string,
    check_version,
)

from helpers import FakeResponse, FakeSession

BASE = "http://core:8099"


def test_api_major_reads_the_canonical_api_object() -> None:
    assert _api_major({"api": {"major": 1, "minor": 0}}) == 1


def test_api_major_tolerates_the_api_version_alias() -> None:
    # A core that still emits the flat key keeps working.
    assert _api_major({"api_version": {"major": 1, "minor": 0}}) == 1
    assert _api_major({"api_version": 1}) == 1


def test_api_version_string_prefers_the_canonical_object() -> None:
    assert api_version_string({"api": {"major": 1, "minor": 4}}) == "1.4"
    assert api_version_string({"api_version": "1.2"}) == "1.2"
    assert api_version_string({}) == ""


def test_check_version_accepts_the_canonical_api_object() -> None:
    # Must not raise.
    check_version({"api": {"major": 1, "minor": 0}, "min_component_version": "0.1.0"})


def test_check_version_rejects_a_newer_major_from_api_object() -> None:
    with pytest.raises(AquilaVersionError):
        check_version({"api": {"major": 2, "minor": 0}})


@pytest.mark.asyncio
async def test_validate_accepts_api_object_and_instance_id() -> None:
    health = {"instance_id": "abc-123", "api": {"major": 1, "minor": 0}}
    session = FakeSession().add(
        "GET", f"{BASE}/api/v1/health", FakeResponse(200, health)
    )
    result = await AquilaApiClient(session, BASE).async_validate()
    assert result == health


@pytest.mark.asyncio
async def test_validate_rejects_a_newer_major_from_api_object() -> None:
    health = {"instance_id": "abc-123", "api": {"major": 2, "minor": 0}}
    session = FakeSession().add(
        "GET", f"{BASE}/api/v1/health", FakeResponse(200, health)
    )
    with pytest.raises(AquilaVersionError):
        await AquilaApiClient(session, BASE).async_validate()


@pytest.mark.asyncio
async def test_validate_raises_when_instance_id_is_missing() -> None:
    health = {"api": {"major": 1, "minor": 0}}
    session = FakeSession().add(
        "GET", f"{BASE}/api/v1/health", FakeResponse(200, health)
    )
    with pytest.raises(AquilaError):
        await AquilaApiClient(session, BASE).async_validate()
