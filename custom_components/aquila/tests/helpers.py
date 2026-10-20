"""Shared test helpers for the Aquila component.

These stubs let the component's firmware logic run under plain ``pytest`` with
no Home Assistant runtime: a fake ``aiohttp`` session for the API client, a stub
coordinator for entities, and a stub client for the snapshot merge. The real HA
base classes (``CoordinatorEntity``, ``UpdateEntity``) are used unchanged; only
their collaborators are stubbed.
"""

from __future__ import annotations

from types import SimpleNamespace
from typing import Any

import aiohttp

from custom_components.aquila.coordinator import AquilaRuntimeData
from custom_components.aquila.update import AquilaFirmwareUpdateEntity


class FakeResponse:
    """A minimal stand-in for ``aiohttp.ClientResponse`` as an async context."""

    def __init__(
        self,
        status: int = 200,
        payload: Any = None,
        text: str = "",
    ) -> None:
        self.status = status
        self._payload = payload
        self._text = text

    async def json(self, content_type: str | None = None) -> Any:
        return self._payload

    async def text(self) -> str:
        return self._text

    async def __aenter__(self) -> "FakeResponse":
        return self

    async def __aexit__(self, *exc: object) -> bool:
        return False


class FakeSession:
    """A fake ``aiohttp.ClientSession`` that records requests by (method, url)."""

    def __init__(self) -> None:
        self.routes: dict[tuple[str, str], Any] = {}
        self.calls: list[SimpleNamespace] = []

    def add(self, method: str, url: str, response: Any) -> "FakeSession":
        self.routes[(method, url)] = response
        return self

    def request(self, method: str, url: str, **kwargs: Any) -> Any:
        self.calls.append(SimpleNamespace(method=method, url=url, **kwargs))
        route = self.routes.get((method, url))
        if route is None:
            raise AssertionError(f"unexpected request: {method} {url}")
        if isinstance(route, Exception):
            raise route
        return route


class StubCoordinator:
    """A coordinator stand-in exposing the surface entities touch."""

    def __init__(self, data: dict[str, Any], success: bool = True) -> None:
        self.data = data
        self.last_update_success = success
        self.refresh_calls = 0

    def async_add_listener(self, update_callback: Any) -> Any:
        return lambda: None

    async def async_request_refresh(self) -> None:
        self.refresh_calls += 1


class StubClient:
    """An ``AquilaApiClient`` stand-in recording firmware calls."""

    def __init__(self, fail: Exception | None = None) -> None:
        self.install_calls: list[tuple[str, str, bool]] = []
        self.abort_calls: list[str] = []
        self._fail = fail

    async def async_install_firmware(
        self, node: str, image_id: str, *, force: bool = False
    ) -> dict[str, Any]:
        if self._fail is not None:
            raise self._fail
        self.install_calls.append((node, image_id, force))
        return {"jobId": "fw-1", "state": "running"}

    async def async_abort_firmware(self, node: str) -> dict[str, Any]:
        if self._fail is not None:
            raise self._fail
        self.abort_calls.append(node)
        return {}


class StubSnapshotClient:
    """A client stand-in for ``async_collect_snapshot`` (no HTTP)."""

    def __init__(
        self,
        *,
        state: Any,
        nodes: Any,
        manifest: Any,
        firmware: dict[str, Any] | None = None,
        jobs: dict[str, Any] | None = None,
        health: Any = None,
        fail_nodes: bool = False,
        fail_health: bool = False,
        fail_firmware_for: set[str] | None = None,
    ) -> None:
        self._state = state
        self._nodes = nodes
        self._manifest = manifest
        self._firmware = firmware or {}
        self._jobs = jobs or {}
        self._health = health if health is not None else {}
        self._fail_nodes = fail_nodes
        self._fail_health = fail_health
        self._fail_firmware_for = fail_firmware_for or set()

    async def async_get_state(self) -> Any:
        return self._state

    async def async_get_health(self) -> Any:
        if self._fail_health:
            from custom_components.aquila.api import AquilaConnectionError

            raise AquilaConnectionError("health probe down")
        return self._health

    async def async_get_nodes(self) -> Any:
        if self._fail_nodes:
            from custom_components.aquila.api import AquilaConnectionError

            raise AquilaConnectionError("nodes down")
        return self._nodes

    async def async_get_firmware_manifest(self) -> Any:
        return self._manifest

    async def async_get_node_firmware(self, node: str) -> Any:
        if node in self._fail_firmware_for:
            from custom_components.aquila.api import AquilaConnectionError

            raise AquilaConnectionError("node firmware down")
        return self._firmware[node]

    async def async_get_firmware_job(self, node: str) -> Any:
        return self._jobs.get(node)


def make_runtime(
    coordinator: StubCoordinator,
    client: Any,
    instance_id: str = "instance-1",
) -> AquilaRuntimeData:
    """Build a runtime for entity construction under the stubs."""
    return AquilaRuntimeData(
        client=client,
        coordinator=coordinator,  # type: ignore[arg-type]
        instance_id=instance_id,
        api_version="1.0",
        min_component_version=None,
        update_interval=60,
        expose_floors=True,
        expose_room_vitals=True,
    )


def make_entity(
    firmware: dict[str, Any],
    *,
    name: str = "Kitchen",
    manifest: dict[str, Any] | None = None,
    success: bool = True,
    fail: Exception | None = None,
) -> tuple[AquilaFirmwareUpdateEntity, StubCoordinator, StubClient]:
    """Construct a real update entity over a stub coordinator and client."""
    node = {"mac": "aa:bb:cc:dd:ee:ff", "name": name, "firmware": firmware}
    data: dict[str, Any] = {
        "nodes": {node["mac"]: node},
        "firmware_manifest": manifest if manifest is not None else {"items": []},
    }
    coordinator = StubCoordinator(data, success=success)
    client = StubClient(fail=fail)
    runtime = make_runtime(coordinator, client)
    entity = AquilaFirmwareUpdateEntity(coordinator, runtime, node["mac"], node)
    return entity, coordinator, client


__all__ = [
    "FakeResponse",
    "FakeSession",
    "StubClient",
    "StubCoordinator",
    "StubSnapshotClient",
    "aiohttp",
    "make_entity",
    "make_runtime",
]
