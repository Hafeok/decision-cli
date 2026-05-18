"""Minimal SSE client for the daemon-mode dispatch subscription (FT-004).

The harness emits ``dispatch`` events through the FT-004 SSE router with
JSON envelopes (``EventEnvelope`` in oxi-events). For slice 1 we are
deliberately small: a blocking ``httpx`` stream that yields one event at
a time. The worker filters for envelopes whose payload targets the
``code-writer`` role.

Use :func:`stream_dispatches` from the worker entry point. For the
end-to-end TC the harness invokes the worker in one-shot mode instead,
so the daemon path is exercised but not required for TC-008.
"""

from __future__ import annotations

import json
from collections.abc import Iterator
from typing import Any

import httpx


def _parse_sse_block(block: str) -> dict[str, Any] | None:
    """Parse one SSE record (lines separated by blank-line)."""
    event_name = "message"
    data_lines: list[str] = []
    for raw in block.splitlines():
        if not raw or raw.startswith(":"):
            continue
        field, _, value = raw.partition(":")
        if value.startswith(" "):
            value = value[1:]
        if field == "event":
            event_name = value
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
    return payload


def stream_dispatches(url: str, *, timeout: float = 30.0) -> Iterator[dict[str, Any]]:
    """Yield ``dispatch`` envelopes from ``url`` as dicts.

    The caller is expected to handle reconnection if the stream drops —
    slice 1's TC-011 covers steady-state delivery only.
    """
    with httpx.Client(timeout=timeout) as client, client.stream(
        "GET",
        url,
        headers={"Accept": "text/event-stream"},
    ) as response:
        response.raise_for_status()
        buffer = ""
        for chunk in response.iter_text():
            buffer += chunk
            while "\n\n" in buffer:
                block, buffer = buffer.split("\n\n", 1)
                parsed = _parse_sse_block(block)
                if parsed is None:
                    continue
                if parsed.get("_event") != "dispatch":
                    continue
                yield parsed
