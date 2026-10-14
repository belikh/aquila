"""Async REST client for the Aquila core's private HTTP API.

Speaks only the paths listed in ``docs/spec/home-assistant.md`` §5.2 (the
authoritative contract for this component). The core's own richer surface in
``docs/spec/core-api.md`` is the underlying implementation; the component uses
this narrow, version-gated subset.

No third-party dependency: ``aiohttp`` is shipped by Home Assistant
(``requirements: []``).
"""

from __future__ import annotations

import asyncio
import logging
from typing import Any

import aiohttp

from .const import (
    API_MAJOR,
    COMPONENT_VERSION,
    HEALTH_API_VERSION,
    HEALTH_INSTANCE_ID,
    HEALTH_MIN_COMPONENT_VERSION,
)

_LOGGER = logging.getLogger(__name__)

_REQUEST_TIMEOUT = aiohttp.ClientTimeout(total=10)


class AquilaError(Exception):
    """Base class for Aquila API errors."""


class AquilaConnectionError(AquilaError):
    """The core could not be reached, or the request timed out."""


class AquilaAuthError(AquilaError):
    """The core rejected the credentials (401)."""


class AquilaVersionError(AquilaError):
    """The core's API or minimum component version is incompatible."""


class AquilaApiError(AquilaError):
    """The core returned a structured error response."""

    def __init__(self, status: int, message: str) -> None:
        super().__init__(f"HTTP {status}: {message}")
        self.status = status
        self.message = message


def parse_version(value: Any) -> tuple[int, ...]:
    """Parse a dotted version string into an integer tuple.

    Unknown or malformed segments contribute zero so a comparison still yields a
    deterministic (if conservative) result rather than raising.
    """
    if isinstance(value, (int, float)):
        value = str(value)
    if not isinstance(value, str):
        return (0,)
    parts: list[int] = []
    for chunk in value.split("."):
        digits = "".join(ch for ch in chunk if ch.isdigit())
        parts.append(int(digits) if digits else 0)
    return tuple(parts) or (0,)


#: Backwards-compatible private alias (used inside this module).
_parse_version = parse_version


def _api_major(health: dict[str, Any]) -> int | None:
    """Extract the API major from either a scalar or ``{major, minor}`` shape."""
    raw = health.get(HEALTH_API_VERSION)
    if isinstance(raw, dict):
        raw = raw.get("major")
    if raw is None:
        return None
    try:
        return int(raw)
    except (TypeError, ValueError):
        return _parse_version(raw)[0]


def check_version(health: dict[str, Any]) -> None:
    """Validate the core version contract; raise :class:`AquilaVersionError`.

    Version gate (home-assistant.md §9): a core that is too new or too old is
    reported clearly rather than silently mismatched.
    """
    api_major = _api_major(health)
    if api_major is not None and api_major != API_MAJOR:
        raise AquilaVersionError(
            f"core API major {api_major} is incompatible with component API major {API_MAJOR}"
        )

    minimum = health.get(HEALTH_MIN_COMPONENT_VERSION)
    if minimum is not None and _parse_version(minimum) > _parse_version(COMPONENT_VERSION):
        raise AquilaVersionError(
            f"core requires component >= {minimum}, this component is {COMPONENT_VERSION}"
        )


class AquilaApiClient:
    """A thin, typed wrapper over the core's private REST API."""

    def __init__(
        self,
        session: aiohttp.ClientSession,
        base_url: str,
        token: str | None = None,
    ) -> None:
        self._session = session
        self._base_url = base_url.rstrip("/")
        self._token = token or None

    @property
    def session(self) -> aiohttp.ClientSession:
        """The shared ``aiohttp`` session."""
        return self._session

    @property
    def base_url(self) -> str:
        """The configured core base URL."""
        return self._base_url

    @property
    def token(self) -> str | None:
        """The configured API token, if any."""
        return self._token

    def auth_headers(self) -> dict[str, str]:
        """Headers carrying the optional bearer token.

        core-api.md §3.3 specifies ``Authorization: Bearer <token>``. The
        loopback/ingress trust boundary is the default credential, so no token
        is sent unless one is configured; a token is only needed when the API is
        reached outside the add-on's private network. A token is never placed in
        a query string (the core rejects that with ``401``).
        """
        if self._token:
            return {"Authorization": f"Bearer {self._token}"}
        return {}

    async def _request(
        self,
        method: str,
        path: str,
        *,
        json: dict[str, Any] | None = None,
    ) -> Any:
        url = f"{self._base_url}{path}"
        headers = self.auth_headers()
        try:
            async with self._session.request(
                method,
                url,
                headers=headers,
                json=json,
                timeout=_REQUEST_TIMEOUT,
            ) as response:
                if response.status == 401:
                    raise AquilaAuthError("core rejected the credentials")
                if response.status >= 400:
                    body = await response.text()
                    raise AquilaApiError(response.status, body)
                if response.status == 204:
                    return None
                return await response.json(content_type=None)
        except aiohttp.ClientError as err:
            raise AquilaConnectionError(str(err)) from err
        except asyncio.TimeoutError as err:
            raise AquilaConnectionError("request to core timed out") from err

    async def async_get_health(self) -> dict[str, Any]:
        """``GET /api/v1/health``."""
        return await self._request("GET", "/api/v1/health")

    async def async_validate(self) -> dict[str, Any]:
        """Fetch and version-check ``/api/v1/health``; return the health body."""
        health = await self.async_get_health()
        check_version(health)
        if not health.get(HEALTH_INSTANCE_ID):
            raise AquilaError("core health response is missing instance_id")
        return health

    async def async_get_schema(self) -> dict[str, Any]:
        """``GET /api/v1/schema`` — zone tree, metric vocabulary, units."""
        return await self._request("GET", "/api/v1/schema")

    async def async_get_state(self) -> dict[str, Any]:
        """``GET /api/v1/state`` — the full current snapshot."""
        return await self._request("GET", "/api/v1/state")

    async def async_get_nodes(self) -> dict[str, Any]:
        """``GET /api/v1/nodes`` — per-node identity and online state."""
        return await self._request("GET", "/api/v1/nodes")

    async def async_get_firmware_manifest(self) -> dict[str, Any]:
        """``GET /api/v1/firmware`` — the firmware image store manifest."""
        return await self._request("GET", "/api/v1/firmware")

    async def async_get_node_firmware(self, node: str) -> dict[str, Any]:
        """``GET /api/v1/nodes/{node}/firmware``."""
        return await self._request("GET", f"/api/v1/nodes/{node}/firmware")

    async def async_install_firmware(
        self,
        node: str,
        image_id: str,
        *,
        force: bool = False,
    ) -> dict[str, Any]:
        """``POST /api/v1/nodes/{node}/firmware/install`` — begin an OTA."""
        return await self._request(
            "POST",
            f"/api/v1/nodes/{node}/firmware/install",
            json={"image_id": image_id, "force": force},
        )

    async def async_get_firmware_job(self, node: str) -> dict[str, Any]:
        """``GET /api/v1/nodes/{node}/firmware/job`` — OTA progress/state."""
        return await self._request("GET", f"/api/v1/nodes/{node}/firmware/job")

    async def async_abort_firmware(self, node: str) -> dict[str, Any]:
        """``POST /api/v1/nodes/{node}/firmware/abort`` — abort an OTA."""
        return await self._request(
            "POST",
            f"/api/v1/nodes/{node}/firmware/abort",
            json={},
        )
