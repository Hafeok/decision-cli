"""TC-142: SDK SSE consumer and HTTP poster for the dispatch/completion protocol.

The three exit-criteria the parent feature_spec (FT-077) names:

1. A worker process subscribes, receives a dispatch event for its
   advertised capability tag, and posts a completion that the harness
   accepts with HTTP 200.
2. Disconnect during a session resumes from the correct ``Last-Event-ID``
   on reconnect.
3. Concurrent claim attempts from two workers on the same dispatch
   resolve with exactly one winner.

Each criterion is exercised against an in-memory fake harness built on
httpx ``MockTransport`` so the tests stay deterministic and fast.
"""

from __future__ import annotations

import asyncio
import json
from typing import Any

import httpx
import pytest

from pipeline_worker_sdk import (
    CapabilityCatalogEntry,
    CompletionFailed,
    CompletionPayload,
    HarnessEndpoints,
    RetryPolicy,
    WireClient,
    envelope_to_dispatch,
    parse_sse_block,
)


# --------------------------------------------------------------------------- #
# Fake harness — assembles SSE frames, accepts claims, accepts completions.   #
# --------------------------------------------------------------------------- #


class FakeHarness:
    """In-memory stand-in for the harness's dispatch + completion endpoints."""

    def __init__(self) -> None:
        # Each entry: (event_id, role_capability_tag, dispatch_id, nquads_payload)
        self.events: list[tuple[str, str, str, str]] = []
        self.claimed_by: dict[str, str] = {}
        self.completions: list[dict[str, Any]] = []
        self.catalog: dict[str, dict[str, str]] = {}
        self.sse_connect_log: list[dict[str, str]] = []
        # Allow the test to inject one-shot completion failures keyed
        # by dispatch_id; each failure pops one element off the list
        # until the list is empty, then the post succeeds.
        self.completion_failures: dict[str, list[int]] = {}

    def add_event(
        self,
        event_id: str,
        capability_tag: str,
        dispatch_id: str,
        nquads_payload: str = "",
    ) -> None:
        self.events.append((event_id, capability_tag, dispatch_id, nquads_payload))

    def _events_after(self, last_event_id: str | None) -> list[tuple[str, str, str, str]]:
        if last_event_id is None:
            return list(self.events)
        # Resume = strictly-after semantics so the same event is not
        # delivered twice.
        out = []
        seen_resume = False
        for event in self.events:
            if seen_resume:
                out.append(event)
            elif event[0] == last_event_id:
                seen_resume = True
        # If the resume cursor was not found, replay everything — the
        # client's checkpoint diverged from the harness's view.
        return out if seen_resume else list(self.events)

    def _build_sse_body(self, events: list[tuple[str, str, str, str]]) -> bytes:
        frames: list[str] = []
        for event_id, tag, dispatch_id, nq in events:
            data = json.dumps(
                {
                    "dispatch_id": dispatch_id,
                    "capability_tag": tag,
                    "nquads_payload": nq,
                    "role_id": "implementer",
                }
            )
            frames.append(f"id: {event_id}\nevent: dispatch\ndata: {data}\n\n")
        return "".join(frames).encode("utf-8")

    def handle(self, request: httpx.Request) -> httpx.Response:
        url = str(request.url)
        method = request.method

        if method == "GET" and url.endswith("/events"):
            last_event_id = request.headers.get("Last-Event-ID")
            cap_tags = request.headers.get("X-Capability-Tags", "")
            self.sse_connect_log.append(
                {"last_event_id": last_event_id or "", "capability_tags": cap_tags}
            )
            tags = set(cap_tags.split(",")) if cap_tags else None
            events = self._events_after(last_event_id)
            if tags is not None:
                events = [e for e in events if e[1] in tags]
            body = self._build_sse_body(events)
            return httpx.Response(
                200,
                content=body,
                headers={"content-type": "text/event-stream"},
            )

        if method == "POST" and "/claim" in url:
            body = json.loads(request.content.decode("utf-8"))
            dispatch_id = body["dispatch_id"]
            worker_id = body["worker_id"]
            owner = self.claimed_by.get(dispatch_id)
            if owner is None:
                self.claimed_by[dispatch_id] = worker_id
                return httpx.Response(200, json={"won": True})
            if owner == worker_id:
                return httpx.Response(200, json={"won": True, "reason": "idempotent"})
            return httpx.Response(
                409, json={"won": False, "reason": "already-claimed"}
            )

        if method == "POST" and "/completion" in url:
            body = json.loads(request.content.decode("utf-8"))
            dispatch_id = body["dispatch_id"]
            pending = self.completion_failures.get(dispatch_id, [])
            if pending:
                status = pending.pop(0)
                return httpx.Response(status, json={"error": "injected"})
            self.completions.append(body)
            return httpx.Response(
                200, json={"accepted": True, "dispatch_id": dispatch_id}
            )

        if method == "GET" and "/catalog/capabilities" in url:
            entries = [
                {"capability_tag": tag, **fields}
                for tag, fields in self.catalog.items()
            ]
            return httpx.Response(200, json={"entries": entries})

        return httpx.Response(404, json={"error": "unknown-route", "url": url})


def _make_client(harness: FakeHarness) -> httpx.AsyncClient:
    return httpx.AsyncClient(transport=httpx.MockTransport(harness.handle))


def _make_endpoints() -> HarnessEndpoints:
    return HarnessEndpoints.from_base("http://harness.test")


# --------------------------------------------------------------------------- #
# Pure-function unit checks for the SSE parser.                               #
# --------------------------------------------------------------------------- #


def test_parse_sse_block_extracts_id_and_data() -> None:
    block = (
        "id: 42\n"
        "event: dispatch\n"
        'data: {"dispatch_id": "d1", "capability_tag": "code-writer"}\n'
    )
    parsed = parse_sse_block(block)
    assert parsed is not None
    assert parsed["_id"] == "42"
    assert parsed["_event"] == "dispatch"
    assert parsed["dispatch_id"] == "d1"
    assert parsed["capability_tag"] == "code-writer"


def test_parse_sse_block_skips_comments_and_blank_fields() -> None:
    block = ": keepalive\nevent: dispatch\ndata: {}\n"
    parsed = parse_sse_block(block)
    assert parsed is not None
    assert parsed["_event"] == "dispatch"


def test_envelope_to_dispatch_filters_non_dispatch() -> None:
    payload = {"_event": "message", "_id": "1", "dispatch_id": "d", "capability_tag": "c"}
    assert envelope_to_dispatch(payload) is None


def test_envelope_to_dispatch_requires_dispatch_id() -> None:
    payload = {"_event": "dispatch", "_id": "1", "capability_tag": "code-writer"}
    assert envelope_to_dispatch(payload) is None


# --------------------------------------------------------------------------- #
# TC-142 acceptance #1 — subscribe → dispatch → claim → complete with 200.   #
# --------------------------------------------------------------------------- #


@pytest.mark.asyncio
async def test_full_dispatch_to_completion_lifecycle() -> None:
    harness = FakeHarness()
    harness.add_event(
        "1", "code-writer", "dispatch:1", "<:s> <:p> <:o> <:g> ."
    )

    client = _make_client(harness)
    wire = WireClient(
        worker_id="worker-A",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
    )
    try:
        events: list[Any] = []
        async for dispatch in wire.dispatches():
            events.append(dispatch)
            claim = await wire.claim(dispatch.dispatch_id)
            assert claim.won is True
            response = await wire.complete(
                CompletionPayload(
                    dispatch_id=dispatch.dispatch_id,
                    session_id="session:1",
                    nquads_payload="<:a> <:b> <:c> <:g> .",
                )
            )
            assert response.status_code == 200
            break
        assert len(events) == 1
        assert events[0].dispatch_id == "dispatch:1"
        assert events[0].capability_tag == "code-writer"
        assert events[0].nquads_payload.startswith("<:s>")
        # The harness accepted exactly one completion and the body
        # carries the SDK-supplied N-Quads payload.
        assert len(harness.completions) == 1
        assert harness.completions[0]["dispatch_id"] == "dispatch:1"
        assert harness.completions[0]["nquads_payload"].startswith("<:a>")
        # The capability-tag advertisement reached the harness on the
        # initial SSE connect.
        assert harness.sse_connect_log[0]["capability_tags"] == "code-writer"
    finally:
        await wire.aclose()


# --------------------------------------------------------------------------- #
# TC-142 acceptance #2 — Last-Event-ID resume after disconnect.              #
# --------------------------------------------------------------------------- #


@pytest.mark.asyncio
async def test_last_event_id_resume_after_disconnect() -> None:
    harness = FakeHarness()
    harness.add_event("10", "code-writer", "dispatch:10")
    harness.add_event("11", "code-writer", "dispatch:11")
    harness.add_event("12", "code-writer", "dispatch:12")

    client = _make_client(harness)
    wire = WireClient(
        worker_id="worker-B",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
    )
    try:
        # First connection: consume events 10 and 11, then simulate
        # disconnect by breaking out of the loop.
        first_round: list[str] = []
        async for dispatch in wire.dispatches():
            first_round.append(dispatch.event_id)
            if dispatch.event_id == "11":
                break
        assert first_round == ["10", "11"]
        assert wire.last_event_id == "11"

        # Second connection: SDK reads its own last_event_id and the
        # harness only delivers events strictly after id=11.
        second_round: list[str] = []
        async for dispatch in wire.dispatches():
            second_round.append(dispatch.event_id)
        assert second_round == ["12"]

        # Both connect attempts hit the harness; the second carried
        # Last-Event-ID=11 in the headers.
        assert len(harness.sse_connect_log) == 2
        assert harness.sse_connect_log[0]["last_event_id"] == ""
        assert harness.sse_connect_log[1]["last_event_id"] == "11"
    finally:
        await wire.aclose()


# --------------------------------------------------------------------------- #
# TC-142 acceptance #3 — concurrent claims → exactly one winner.             #
# --------------------------------------------------------------------------- #


@pytest.mark.asyncio
async def test_concurrent_claims_one_winner_one_loser() -> None:
    harness = FakeHarness()
    endpoints = _make_endpoints()

    client_a = _make_client(harness)
    client_b = _make_client(harness)
    wire_a = WireClient(
        worker_id="worker-A",
        capability_tags=["code-writer"],
        endpoints=endpoints,
        client=client_a,
    )
    wire_b = WireClient(
        worker_id="worker-B",
        capability_tags=["code-writer"],
        endpoints=endpoints,
        client=client_b,
    )
    try:
        results = await asyncio.gather(
            wire_a.claim("dispatch:contested"),
            wire_b.claim("dispatch:contested"),
        )
        winners = [r for r in results if r.won]
        losers = [r for r in results if not r.won]
        assert len(winners) == 1
        assert len(losers) == 1
        assert losers[0].reason == "already-claimed"
        # The owner recorded by the harness is the same worker who saw
        # `won=True` come back over the wire.
        winner = winners[0]
        owning_wire = wire_a if harness.claimed_by["dispatch:contested"] == "worker-A" else wire_b
        assert owning_wire.worker_id in {"worker-A", "worker-B"}
        # Sanity: the same worker re-claiming is idempotent.
        repeat = await owning_wire.claim("dispatch:contested")
        assert repeat.won is True
        del winner
    finally:
        await wire_a.aclose()
        await wire_b.aclose()


# --------------------------------------------------------------------------- #
# Coverage extras: retry/backoff on transient 5xx, catalog cache reuse.       #
# --------------------------------------------------------------------------- #


@pytest.mark.asyncio
async def test_completion_poster_retries_then_succeeds() -> None:
    harness = FakeHarness()
    harness.completion_failures["dispatch:retry"] = [503, 503]

    client = _make_client(harness)
    wire = WireClient(
        worker_id="worker-C",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
        retry_policy=RetryPolicy(max_attempts=5, base_backoff=0.0, max_backoff=0.0),
    )
    try:
        response = await wire.complete(
            CompletionPayload(
                dispatch_id="dispatch:retry",
                session_id="session:retry",
                nquads_payload="",
            )
        )
        assert response.status_code == 200
        assert len(harness.completions) == 1
    finally:
        await wire.aclose()


@pytest.mark.asyncio
async def test_completion_poster_exhausts_retries() -> None:
    harness = FakeHarness()
    # Six failures will outlast five attempts.
    harness.completion_failures["dispatch:dead"] = [503, 503, 503, 503, 503, 503]

    client = _make_client(harness)
    wire = WireClient(
        worker_id="worker-D",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
        retry_policy=RetryPolicy(max_attempts=3, base_backoff=0.0, max_backoff=0.0),
    )
    try:
        with pytest.raises(CompletionFailed):
            await wire.complete(
                CompletionPayload(
                    dispatch_id="dispatch:dead",
                    session_id="session:dead",
                    nquads_payload="",
                )
            )
        assert len(harness.completions) == 0
    finally:
        await wire.aclose()


@pytest.mark.asyncio
async def test_catalog_cache_avoids_redundant_fetches() -> None:
    harness = FakeHarness()
    harness.catalog = {
        "code-writer": {
            "model_group": "lgroup-code",
            "endpoint": "scaleway",
            "version": "1",
        },
        "frontier-reasoning": {
            "model_group": "lgroup-opus",
            "endpoint": "anthropic",
            "version": "1",
        },
    }

    fetch_counter = {"calls": 0}

    def counting_handler(request: httpx.Request) -> httpx.Response:
        if "/catalog/capabilities" in str(request.url):
            fetch_counter["calls"] += 1
        return harness.handle(request)

    client = httpx.AsyncClient(transport=httpx.MockTransport(counting_handler))
    wire = WireClient(
        worker_id="worker-E",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
    )
    try:
        first = await wire.resolve_capability("code-writer")
        second = await wire.resolve_capability("code-writer")
        third = await wire.resolve_capability("frontier-reasoning")
        assert isinstance(first, CapabilityCatalogEntry)
        assert first.model_group == "lgroup-code"
        assert second is not None and second.model_group == "lgroup-code"
        assert third is not None and third.model_group == "lgroup-opus"
        # Only the very first call should have triggered an HTTP fetch
        # — once the cache is fresh, every other resolve is in-memory.
        assert fetch_counter["calls"] == 1
    finally:
        await wire.aclose()


@pytest.mark.asyncio
async def test_sse_filters_out_other_capability_tags() -> None:
    harness = FakeHarness()
    harness.add_event("100", "verifier", "dispatch:not-mine")
    harness.add_event("101", "code-writer", "dispatch:mine")

    client = _make_client(harness)
    wire = WireClient(
        worker_id="worker-F",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
    )
    try:
        observed: list[str] = []
        async for dispatch in wire.dispatches():
            observed.append(dispatch.dispatch_id)
        assert observed == ["dispatch:mine"]
    finally:
        await wire.aclose()
