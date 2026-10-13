"""WebSocket client for the Aquila core stream.

Owns the connection, reconnect and exponential backoff, and ``seq`` handling
(home-assistant.md §5.2/§6.2). It transports nothing itself: each accepted
message is handed to the coordinator's ``apply_event``.

Contract notes:
- ``seq`` is monotonic per connection; a message whose ``seq`` is not greater
  than the last applied one is dropped as stale.
- A ``seq`` gap forces an HTTP resync.
- A disconnected stream marks the core unreachable; entities become
  unavailable until reconnect.
- The server's own envelope (``type``/``seq``/``ts`` plus payload fields) is the
  home-assistant.md shape. A ``data`` sub-object, if present, is tolerated and
  unwrapped so the client also accepts the core-api.md envelope.
"""

from __future__ import annotations

import asyncio
import json
import logging
import random
from collections.abc import Callable
from contextlib import suppress
from typing import Any
from urllib.parse import urlsplit, urlunsplit

import aiohttp

from .api import AquilaApiClient
from .const import (
    EVENT_TYPE_FIRMWARE,
    EVENT_TYPE_HEALTH,
    EVENT_TYPE_STATE,
    WS_HEARTBEAT_S,
    WS_PATH,
    WS_RECONNECT_BASE,
    WS_RECONNECT_FACTOR,
    WS_RECONNECT_MAX,
    WS_SUBPROTOCOL,
)

_LOGGER = logging.getLogger(__name__)

EventCallback = Callable[[dict[str, Any]], None]
StateCallback = Callable[[], None]


class AquilaStreamClient:
    """A reconnecting ``aiohttp`` WebSocket client for ``/api/v1/stream``."""

    def __init__(
        self,
        hass: Any,
        client: AquilaApiClient,
        *,
        on_event: EventCallback,
        on_connected: StateCallback,
        on_disconnected: StateCallback,
        on_gap: StateCallback,
    ) -> None:
        self._hass = hass
        self._client = client
        self._on_event = on_event
        self._on_connected = on_connected
        self._on_disconnected = on_disconnected
        self._on_gap = on_gap
        self._task: asyncio.Task[None] | None = None
        self._ws: aiohttp.ClientWebSocketResponse | None = None
        self._stopping = False
        self._last_seq = 0

    @property
    def connected(self) -> bool:
        """Whether a live WebSocket currently exists."""
        return self._ws is not None and not self._ws.closed

    async def async_start(self) -> None:
        """Start the background reconnect loop (idempotent)."""
        if self._task is not None:
            return
        self._stopping = False
        self._task = self._hass.async_create_background_task(
            self._run(), name="aquila_stream"
        )

    async def async_stop(self) -> None:
        """Stop the loop and close the socket."""
        self._stopping = True
        if self._ws is not None:
            with suppress(Exception):
                await self._ws.close()
        if self._task is not None:
            self._task.cancel()
            with suppress(asyncio.CancelledError):
                await self._task
            self._task = None
        self._ws = None

    async def _run(self) -> None:
        delay = WS_RECONNECT_BASE
        while not self._stopping:
            try:
                await self._connect_once()
                # A clean close resets the backoff.
                delay = WS_RECONNECT_BASE
            except asyncio.CancelledError:
                raise
            except Exception as err:  # noqa: BLE001 - reconnect on any failure
                _LOGGER.debug("Aquila stream connection error: %s", err)

            if self._stopping:
                break

            # The connection has ended: reset per-connection state and notify.
            self._last_seq = 0
            self._ws = None
            self._on_disconnected()

            jitter = random.uniform(0.0, delay / 2)
            await asyncio.sleep(min(delay, WS_RECONNECT_MAX) + jitter)
            delay = min(delay * WS_RECONNECT_FACTOR, WS_RECONNECT_MAX)

    async def _connect_once(self) -> None:
        url = self._ws_url()
        async with self._client.session.ws_connect(
            url,
            headers=self._client.auth_headers(),
            protocols=(WS_SUBPROTOCOL,),
            heartbeat=WS_HEARTBEAT_S,
        ) as ws:
            self._ws = ws
            # A fresh connection restarts the sequence at 1; the coordinator
            # must resync so it does not drop new frames as stale.
            self._on_connected()
            async for msg in ws:
                if msg.type == aiohttp.WSMsgType.TEXT:
                    self._handle_text(msg.data)
                elif msg.type == aiohttp.WSMsgType.ERROR:
                    break
                elif msg.type in (
                    aiohttp.WSMsgType.CLOSE,
                    aiohttp.WSMsgType.CLOSING,
                    aiohttp.WSMsgType.CLOSED,
                ):
                    break

    def _ws_url(self) -> str:
        parts = urlsplit(self._client.base_url)
        scheme = "wss" if parts.scheme == "https" else "ws"
        path = (parts.path.rstrip("/") if parts.path else "") + WS_PATH
        return urlunsplit((scheme, parts.netloc, path, "", ""))

    def _handle_text(self, raw: str) -> None:
        try:
            event = json.loads(raw)
        except ValueError:
            _LOGGER.debug("Ignoring non-JSON stream frame")
            return
        if not isinstance(event, dict):
            return

        # Protocol major, when present, must be 1 (core-api.md §10.2).
        if event.get("v") not in (None, 1):
            return

        event = self._normalise(event)
        if event.get("type") not in (
            EVENT_TYPE_STATE,
            EVENT_TYPE_HEALTH,
            EVENT_TYPE_FIRMWARE,
        ):
            # Unknown event types are ignored by contract.
            return

        seq = event.get("seq")
        if isinstance(seq, int):
            if seq <= self._last_seq:
                return  # stale: not greater than the last applied seq
            if self._last_seq and seq > self._last_seq + 1:
                _LOGGER.debug("Aquila stream seq gap (%s -> %s); resyncing", self._last_seq, seq)
                self._on_gap()
            self._last_seq = seq

        self._on_event(event)

    @staticmethod
    def _normalise(event: dict[str, Any]) -> dict[str, Any]:
        """Flatten a core-api-style ``data`` envelope into the flat shape.

        home-assistant.md emits flat payloads; core-api.md nests them under
        ``data`` with ``id``. Unwrapping keeps the coordinator's merge simple.
        """
        data = event.get("data")
        if not isinstance(data, dict):
            return event
        flat = dict(event)
        flat.pop("data", None)
        flat.update(data)
        return flat
