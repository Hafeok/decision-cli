"""SSE consumer for harness dispatch events with Last-Event-ID resume (ADR-045)."""

from __future__ import annotations

import json
from collections.abc import AsyncIterator, Iterable
from typing import Any

import httpx

from .types import DispatchEvent


def parse_sse_block(block: str) -> dict[str, Any] | None:
    """Parse one SSE record (lines separated by blank-line).

    Returns a dict with ``_event``, ``_id``, and ``_data`` keys plus the
    parsed JSON payload (merged in at the top level). Returns ``None`` if
    the block has no `data:` field or is not valid JSON.
    """
    event_name = "message"
    event_id: str | None = None
    data_lines: list[str] = []
    for raw in block.splitlines():
        if not raw or raw.startswith(":"):
            continue
        field, _, value = raw.partition(":")
        if value.startswith(" "):
            value = value[1:]
        if field == "event":
            event_name = value
        elif field == "id":
            event_id = value
        elif field == "data":
            data_lines.append(value)
    if not data_lines:
        return None
    blob = "\n".join(data_lines)
    try:
        payload = json.loads(blob)
    except json.JSONDecodeError:
        return None
    if not isinstance(payload, dict):
        return None
    payload["_event"] = event_name
    if event_id is not None:
        payload["_id"] = event_id
    return payload


def envelope_to_dispatch(payload: dict[str, Any]) -> DispatchEvent | None:
    """Convert a parsed SSE payload into a ``DispatchEvent``.

    Accepts both the legacy oxi-events JSON envelope shape (where the
    capability tag may live under ``payload.capability_tag`` or
    ``capability``) and a flatter SDK-native shape. Returns ``None`` if the
    payload is missing the dispatch_id or capability_tag.
    """
    if payload.get("_event") != "dispatch":
        return None
    event_id = str(payload.get("_id", ""))
    if not event_id:
        # Fall back to seq if the harness puts it in the body.
        seq = payload.get("seq") or payload.get("sequence")
        if seq is None:
            return None
        event_id = str(seq)

    dispatch_id = (
        payload.get("dispatch_id")
        or payload.get("dispatchId")
        or payload.get("dispatch_iri")
    )
    capability_tag = (
        payload.get("capability_tag")
        or payload.get("capabilityTag")
        or payload.get("capability")
    )
    if not dispatch_id or not capability_tag:
        return None

    metadata: dict[str, Any] = {}
    for key in ("role_id", "bundle_hash", "session_id", "stream_id", "seq"):
        if key in payload:
            metadata[key] = payload[key]
    if "metadata" in payload and isinstance(payload["metadata"], dict):
        metadata.update(payload["metadata"])

    nquads_payload = payload.get("nquads_payload") or payload.get("nquadsPayload") or ""

    return DispatchEvent(
        event_id=event_id,
        dispatch_id=str(dispatch_id),
        capability_tag=str(capability_tag),
        nquads_payload=str(nquads_payload),
        metadata=metadata,
    )


class SseConsumer:
    """Long-lived SSE connection to the harness's dispatch endpoint.

    Tracks the most recently delivered event id so reconnects can resume
    via ``Last-Event-ID`` (ADR-045). The consumer filters out any envelope
    whose capability tag is not in the advertised set, so two workers with
    disjoint capabilities never see each other's dispatches.
    """

    def __init__(
        self,
        url: str,
        capability_tags: Iterable[str],
        *,
        client: httpx.AsyncClient | None = None,
        timeout: float = 30.0,
    ) -> None:
        self.url = url
        self.capability_tags = frozenset(capability_tags)
        self._last_event_id: str | None = None
        self._client = client
        self._owns_client = client is None
        self._timeout = timeout

    @property
    def last_event_id(self) -> str | None:
        """Most recent event id seen — used as ``Last-Event-ID`` on resume."""
        return self._last_event_id

    def set_last_event_id(self, event_id: str | None) -> None:
        """Override the resume cursor (used after warm-starting from a checkpoint)."""
        self._last_event_id = event_id

    def _build_headers(self) -> dict[str, str]:
        headers: dict[str, str] = {"Accept": "text/event-stream"}
        if self._last_event_id is not None:
            headers["Last-Event-ID"] = self._last_event_id
        if self.capability_tags:
            # Advertise capability tags on connect so the harness can
            # filter server-side; the SDK still filters client-side as a
            # defense-in-depth check.
            headers["X-Capability-Tags"] = ",".join(sorted(self.capability_tags))
        return headers

    async def _client_or_new(self) -> httpx.AsyncClient:
        if self._client is None:
            self._client = httpx.AsyncClient(timeout=self._timeout)
        return self._client

    async def aclose(self) -> None:
        if self._owns_client and self._client is not None:
            await self._client.aclose()
            self._client = None

    async def stream(self) -> AsyncIterator[DispatchEvent]:
        """Yield ``DispatchEvent`` instances until the stream ends or errors.

        The caller is responsible for re-invoking ``stream()`` on
        reconnect; ``last_event_id`` will carry the resume cursor.
        """
        client = await self._client_or_new()
        async with client.stream(
            "GET", self.url, headers=self._build_headers()
        ) as response:
            response.raise_for_status()
            buffer = ""
            async for chunk in response.aiter_text():
                buffer += chunk
                while "\n\n" in buffer:
                    block, buffer = buffer.split("\n\n", 1)
                    parsed = parse_sse_block(block)
                    if parsed is None:
                        continue
                    dispatch = envelope_to_dispatch(parsed)
                    if dispatch is None:
                        continue
                    self._last_event_id = dispatch.event_id
                    if (
                        self.capability_tags
                        and dispatch.capability_tag not in self.capability_tags
                    ):
                        # Server-side filtering should already have removed
                        # this; keep the client-side guard so a misbehaving
                        # harness cannot fan us out to dispatches we cannot
                        # service.
                        continue
                    yield dispatch
