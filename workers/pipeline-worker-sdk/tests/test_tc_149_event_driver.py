"""TC-149: Production EventDriver wiring SSE+POST to Session lifecycle (FT-084).

The two success criteria the parent feature_spec names:

1. An EventDriver instance, given a harness SSE endpoint, runs one
   dispatch → completion cycle end-to-end against a live pipeline-cli.
2. Transient SSE disconnect mid-session resumes correctly; transient
   POST failure on completion retries with backoff and eventually
   succeeds (or surfaces a permanent failure to the operator).

Plus the structural contract the EventDriver inherits from the Driver
Protocol (FT-083): worker code written against `Driver` runs unchanged
under both `FakeDriver` (TC-148) and `EventDriver` (this suite).

The harness is replaced by an in-memory `httpx.MockTransport` so the
tests stay deterministic and fast — same pattern as TC-142's wire
suite — without losing any coverage of the integration points.
"""

from __future__ import annotations

import asyncio
import json
from typing import Any

import httpx
import pyoxigraph
import pytest

from pipeline_worker_sdk import (
    OUTCOME_BLOCKED,
    OUTCOME_SUCCESS,
    CompletionFailed,
    CompletionPayload,
    CompletionRejected,
    Driver,
    EventDriver,
    HarnessEndpoints,
    RetryPolicy,
    Session,
)


# --------------------------------------------------------------------------- #
# Fake harness — mirrors the TC-142 surface so EventDriver gets a live wire.  #
# --------------------------------------------------------------------------- #


class _SseGate:
    """A coordination primitive so we can hold the SSE response open."""

    def __init__(self) -> None:
        self._event = asyncio.Event()

    def close(self) -> None:
        self._event.set()

    async def wait(self) -> None:
        await self._event.wait()


class FakeHarness:
    """In-memory fake harness exposing SSE + claim + completion + catalog."""

    def __init__(self) -> None:
        # (event_id, capability_tag, dispatch_id, nquads_payload)
        self.events: list[tuple[str, str, str, str]] = []
        self.claimed_by: dict[str, str] = {}
        self.completions: list[dict[str, Any]] = []
        self.sse_connect_log: list[dict[str, str]] = []
        # Per-dispatch list of HTTP statuses to return on completion before
        # falling through to a success (each entry pops one element).
        self.completion_failures: dict[str, list[int]] = {}
        # Per-dispatch HARD rejection status (4xx); raised on every attempt
        # — used to test CompletionRejected propagation.
        self.completion_rejections: dict[str, int] = {}
        # Inject SSE failures: list of failure modes consumed in order on
        # successive GET /events requests. 'http-500' returns a 500 response;
        # 'network' raises a httpx.ConnectError; anything else is a 200.
        self.sse_connect_failures: list[str] = []
        # When True, the first SSE connect responds with an EMPTY body that
        # closes immediately — simulating a clean disconnect mid-stream.
        # Subsequent connects return the events newer than the cursor.
        self.sse_close_after_event_ids: set[str] = set()

    def add_event(
        self,
        event_id: str,
        capability_tag: str,
        dispatch_id: str,
        nquads_payload: str = "",
    ) -> None:
        self.events.append((event_id, capability_tag, dispatch_id, nquads_payload))

    def _events_after(
        self, last_event_id: str | None
    ) -> list[tuple[str, str, str, str]]:
        if last_event_id is None:
            return list(self.events)
        out: list[tuple[str, str, str, str]] = []
        seen = False
        for event in self.events:
            if seen:
                out.append(event)
            elif event[0] == last_event_id:
                seen = True
        return out if seen else list(self.events)

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
            return self._handle_sse(request)
        if method == "POST" and "/claim" in url:
            return self._handle_claim(request)
        if method == "POST" and "/completion" in url:
            return self._handle_completion(request)
        if method == "GET" and "/catalog/capabilities" in url:
            return httpx.Response(200, json={"entries": []})

        return httpx.Response(404, json={"error": "unknown-route", "url": url})

    def _handle_sse(self, request: httpx.Request) -> httpx.Response:
        last_event_id = request.headers.get("Last-Event-ID")
        cap_tags = request.headers.get("X-Capability-Tags", "")
        self.sse_connect_log.append(
            {"last_event_id": last_event_id or "", "capability_tags": cap_tags}
        )
        if self.sse_connect_failures:
            mode = self.sse_connect_failures.pop(0)
            if mode == "http-500":
                return httpx.Response(500, text="upstream-error")
            if mode == "network":
                raise httpx.ConnectError("simulated network failure")
        tags = set(cap_tags.split(",")) if cap_tags else None
        events = self._events_after(last_event_id)
        if tags is not None:
            events = [e for e in events if e[1] in tags]
        # Truncate the SSE body at any close-after-id markers so the
        # connection delivers a prefix of events then closes cleanly.
        truncated: list[tuple[str, str, str, str]] = []
        for ev in events:
            truncated.append(ev)
            if ev[0] in self.sse_close_after_event_ids:
                break
        body = self._build_sse_body(truncated)
        return httpx.Response(
            200, content=body, headers={"content-type": "text/event-stream"}
        )

    def _handle_claim(self, request: httpx.Request) -> httpx.Response:
        body = json.loads(request.content.decode("utf-8"))
        dispatch_id = body["dispatch_id"]
        worker_id = body["worker_id"]
        owner = self.claimed_by.get(dispatch_id)
        if owner is None:
            self.claimed_by[dispatch_id] = worker_id
            return httpx.Response(200, json={"won": True})
        if owner == worker_id:
            return httpx.Response(200, json={"won": True, "reason": "idempotent"})
        return httpx.Response(409, json={"won": False, "reason": "already-claimed"})

    def _handle_completion(self, request: httpx.Request) -> httpx.Response:
        body = json.loads(request.content.decode("utf-8"))
        dispatch_id = body["dispatch_id"]
        if dispatch_id in self.completion_rejections:
            status = self.completion_rejections[dispatch_id]
            return httpx.Response(status, json={"error": "shacl-violation"})
        pending = self.completion_failures.get(dispatch_id, [])
        if pending:
            status = pending.pop(0)
            return httpx.Response(status, json={"error": "injected"})
        self.completions.append(body)
        return httpx.Response(
            200, json={"accepted": True, "dispatch_id": dispatch_id}
        )


def _make_client(harness: FakeHarness) -> httpx.AsyncClient:
    return httpx.AsyncClient(transport=httpx.MockTransport(harness.handle))


def _make_endpoints() -> HarnessEndpoints:
    return HarnessEndpoints.from_base("http://harness.test")


def _zero_policy() -> RetryPolicy:
    return RetryPolicy(max_attempts=5, base_backoff=0.0, max_backoff=0.0)


def _parse_nquads(text: str) -> list[pyoxigraph.Quad]:
    if not text.strip():
        return []
    store = pyoxigraph.Store()
    store.load(input=text, format=pyoxigraph.RdfFormat.N_QUADS)
    return list(store)


# --------------------------------------------------------------------------- #
# Protocol conformance: EventDriver IS-A Driver.                              #
# --------------------------------------------------------------------------- #


def test_event_driver_satisfies_driver_protocol() -> None:
    driver = EventDriver(
        worker_id="protocol-worker",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=_make_client(FakeHarness()),
    )
    assert isinstance(driver, Driver)


# --------------------------------------------------------------------------- #
# Criterion #1 — full dispatch → completion cycle end-to-end.                 #
# --------------------------------------------------------------------------- #


@pytest.mark.asyncio
async def test_full_dispatch_completion_cycle_against_in_memory_harness() -> None:
    harness = FakeHarness()
    harness.add_event(
        "1",
        "code-writer",
        "urn:dec:dispatch:1",
        "<http://x/s> <http://x/p> <http://x/o> <http://x/g> .",
    )
    client = _make_client(harness)

    completions: list[CompletionPayload] = []
    async with EventDriver(
        worker_id="worker-1",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
        retry_policy=_zero_policy(),
    ) as driver:
        async for session in driver:
            assert isinstance(session, Session)
            assert session.dispatch_id == "urn:dec:dispatch:1"
            assert session.bundle_size == 1
            session.emit_artifact_nquads(
                "<http://example.org/artifact/1> "
                "<http://example.org/produced> "
                '"ok" '
                "<http://example.org/g/artifact> ."
            )
            session.record_telemetry("model_group", "lgroup-code")
            payload = session.build_completion()
            await driver.complete(payload)
            completions.append(payload)
            break

    assert len(completions) == 1
    assert completions[0].outcome == OUTCOME_SUCCESS
    assert len(harness.completions) == 1
    received = harness.completions[0]
    assert received["dispatch_id"] == "urn:dec:dispatch:1"
    quads = _parse_nquads(received["nquads_payload"])
    assert len(quads) == 1
    # Capability-tag advertisement reached the harness.
    assert harness.sse_connect_log[0]["capability_tags"] == "code-writer"
    # The dispatch was claimed by this worker before completion was posted.
    assert harness.claimed_by["urn:dec:dispatch:1"] == "worker-1"


# --------------------------------------------------------------------------- #
# Criterion #2a — transient SSE disconnect resumes via Last-Event-ID.         #
# --------------------------------------------------------------------------- #


@pytest.mark.asyncio
async def test_sse_disconnect_resumes_from_last_event_id() -> None:
    harness = FakeHarness()
    harness.add_event("10", "code-writer", "urn:dec:dispatch:10")
    harness.add_event("11", "code-writer", "urn:dec:dispatch:11")
    harness.add_event("12", "code-writer", "urn:dec:dispatch:12")
    # The first SSE connection closes cleanly after event 11; the driver
    # must reconnect transparently and pick up event 12.
    harness.sse_close_after_event_ids.add("11")

    client = _make_client(harness)
    seen: list[str] = []
    async with EventDriver(
        worker_id="worker-resume",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
        retry_policy=_zero_policy(),
    ) as driver:
        async for session in driver:
            seen.append(session.dispatch_id)
            payload = session.build_completion()
            await driver.complete(payload)
            if len(seen) == 3:
                break

    assert seen == [
        "urn:dec:dispatch:10",
        "urn:dec:dispatch:11",
        "urn:dec:dispatch:12",
    ]
    # The harness saw two SSE connect attempts; the second carried the
    # Last-Event-ID cursor (11) so it only replayed event 12.
    assert len(harness.sse_connect_log) >= 2
    assert harness.sse_connect_log[0]["last_event_id"] == ""
    assert harness.sse_connect_log[1]["last_event_id"] == "11"


@pytest.mark.asyncio
async def test_sse_transient_http_error_retries_within_policy() -> None:
    harness = FakeHarness()
    harness.add_event("1", "code-writer", "urn:dec:dispatch:retry-after-5xx")
    # First connect attempt returns 500; second succeeds. The driver's
    # SSE reconnect policy retries automatically with zero backoff.
    harness.sse_connect_failures = ["http-500"]
    client = _make_client(harness)

    seen: list[str] = []
    async with EventDriver(
        worker_id="worker-sse-retry",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
        retry_policy=_zero_policy(),
        sse_reconnect_policy=RetryPolicy(
            max_attempts=4, base_backoff=0.0, max_backoff=0.0
        ),
    ) as driver:
        async for session in driver:
            seen.append(session.dispatch_id)
            await driver.complete(session.build_completion())
            break

    assert seen == ["urn:dec:dispatch:retry-after-5xx"]
    assert len(harness.sse_connect_log) >= 2


@pytest.mark.asyncio
async def test_sse_reconnect_exhaustion_propagates_last_error() -> None:
    harness = FakeHarness()
    harness.sse_connect_failures = ["network", "network", "network"]
    client = _make_client(harness)

    async with EventDriver(
        worker_id="worker-sse-dead",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
        retry_policy=_zero_policy(),
        sse_reconnect_policy=RetryPolicy(
            max_attempts=2, base_backoff=0.0, max_backoff=0.0
        ),
    ) as driver:
        with pytest.raises(httpx.HTTPError):
            async for _session in driver:
                pytest.fail("should not yield a session — every SSE connect failed")


# --------------------------------------------------------------------------- #
# Criterion #2b — transient completion POST failure retries with backoff.    #
# --------------------------------------------------------------------------- #


@pytest.mark.asyncio
async def test_completion_post_retries_then_succeeds() -> None:
    harness = FakeHarness()
    harness.add_event("1", "code-writer", "urn:dec:dispatch:retry")
    harness.completion_failures["urn:dec:dispatch:retry"] = [503, 503]
    client = _make_client(harness)

    async with EventDriver(
        worker_id="worker-post-retry",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
        retry_policy=RetryPolicy(max_attempts=5, base_backoff=0.0, max_backoff=0.0),
    ) as driver:
        async for session in driver:
            payload = session.build_completion()
            await driver.complete(payload)
            break

    # After two 5xx retries the third attempt succeeded — exactly one
    # entry made it into the harness's completions log.
    assert len(harness.completions) == 1
    assert harness.completions[0]["dispatch_id"] == "urn:dec:dispatch:retry"


@pytest.mark.asyncio
async def test_completion_post_surfaces_permanent_failure() -> None:
    harness = FakeHarness()
    harness.add_event("1", "code-writer", "urn:dec:dispatch:dead")
    harness.completion_failures["urn:dec:dispatch:dead"] = [503, 503, 503, 503, 503]
    client = _make_client(harness)

    async with EventDriver(
        worker_id="worker-post-dead",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
        retry_policy=RetryPolicy(max_attempts=2, base_backoff=0.0, max_backoff=0.0),
    ) as driver:
        async for session in driver:
            payload = session.build_completion()
            with pytest.raises(CompletionFailed):
                await driver.complete(payload)
            break

    assert harness.completions == []


@pytest.mark.asyncio
async def test_completion_post_surfaces_deterministic_rejection() -> None:
    harness = FakeHarness()
    harness.add_event("1", "code-writer", "urn:dec:dispatch:rejected")
    harness.completion_rejections["urn:dec:dispatch:rejected"] = 422
    client = _make_client(harness)

    async with EventDriver(
        worker_id="worker-rejection",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
        retry_policy=_zero_policy(),
    ) as driver:
        async for session in driver:
            payload = session.build_completion()
            with pytest.raises(CompletionRejected) as excinfo:
                await driver.complete(payload)
            assert excinfo.value.status == 422
            break

    assert harness.completions == []


# --------------------------------------------------------------------------- #
# Claim contention: lost claims are skipped silently.                         #
# --------------------------------------------------------------------------- #


@pytest.mark.asyncio
async def test_lost_claim_skips_dispatch_silently() -> None:
    harness = FakeHarness()
    harness.add_event("1", "code-writer", "urn:dec:dispatch:contested")
    harness.add_event("2", "code-writer", "urn:dec:dispatch:mine")
    # Pre-claim the contested dispatch by a different worker so our
    # driver loses the claim race when it polls.
    harness.claimed_by["urn:dec:dispatch:contested"] = "other-worker"

    client = _make_client(harness)
    yielded: list[str] = []
    async with EventDriver(
        worker_id="worker-claim",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
        retry_policy=_zero_policy(),
    ) as driver:
        async for session in driver:
            yielded.append(session.dispatch_id)
            await driver.complete(session.build_completion())
            break

    # The lost-claim dispatch never reached worker code; only the second
    # dispatch was yielded as a Session.
    assert yielded == ["urn:dec:dispatch:mine"]
    assert harness.completions[0]["dispatch_id"] == "urn:dec:dispatch:mine"


# --------------------------------------------------------------------------- #
# Lifecycle: aclose is idempotent; closed driver stops yielding.              #
# --------------------------------------------------------------------------- #


@pytest.mark.asyncio
async def test_aclose_is_idempotent() -> None:
    driver = EventDriver(
        worker_id="worker-close",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=_make_client(FakeHarness()),
    )
    await driver.aclose()
    await driver.aclose()
    assert driver.closed is True


@pytest.mark.asyncio
async def test_complete_on_closed_driver_raises() -> None:
    driver = EventDriver(
        worker_id="worker-closed-complete",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=_make_client(FakeHarness()),
    )
    await driver.aclose()
    payload = CompletionPayload(
        dispatch_id="urn:dec:dispatch:test",
        session_id="urn:dec:session:test",
    )
    with pytest.raises(RuntimeError, match="closed"):
        await driver.complete(payload)


@pytest.mark.asyncio
async def test_context_manager_closes_on_clean_exit() -> None:
    harness = FakeHarness()
    client = _make_client(harness)
    driver = EventDriver(
        worker_id="worker-ctx",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
    )
    async with driver:
        assert driver.opened is True
        assert driver.closed is False
    assert driver.closed is True


@pytest.mark.asyncio
async def test_context_manager_posts_blocked_on_worker_crash() -> None:
    """If the worker raises mid-session, EventDriver best-effort POSTs a
    blocked completion so the harness sees the session terminate.
    """
    harness = FakeHarness()
    harness.add_event("1", "code-writer", "urn:dec:dispatch:crash")
    client = _make_client(harness)

    class WorkerCrashed(RuntimeError):
        pass

    driver = EventDriver(
        worker_id="worker-crash",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
        retry_policy=_zero_policy(),
    )
    with pytest.raises(WorkerCrashed):
        async with driver:
            async for session in driver:
                # Worker emits a side-channel feedback note then dies.
                session.emit_side_channel_nquads(
                    "<http://example.org/feedback/crash> "
                    "<http://example.org/feedbackClass> "
                    '"gap" '
                    "<http://example.org/g/side> ."
                )
                raise WorkerCrashed("model timed out")

    assert driver.closed is True
    # Exactly one completion landed — the best-effort blocked one.
    assert len(harness.completions) == 1
    blocked = harness.completions[0]
    assert blocked["outcome"] == OUTCOME_BLOCKED
    # Side-channel triple survived; no half-formed artifact accompanied it.
    quads = _parse_nquads(blocked["nquads_payload"])
    assert len(quads) == 1
    assert "feedback/crash" in str(quads[0].subject)


@pytest.mark.asyncio
async def test_capability_tag_advertised_to_harness() -> None:
    harness = FakeHarness()
    client = _make_client(harness)
    async with EventDriver(
        worker_id="worker-tags",
        capability_tags=["code-writer", "verifier"],
        endpoints=_make_endpoints(),
        client=client,
    ) as driver:
        # Trigger one SSE connect via __anext__; the empty harness will
        # end the stream cleanly with no events to yield.
        with pytest.raises(StopAsyncIteration):
            await driver.__anext__()

    assert harness.sse_connect_log[0]["capability_tags"] in {
        "code-writer,verifier",
        "verifier,code-writer",
    }


@pytest.mark.asyncio
async def test_set_last_event_id_warm_starts_resume_cursor() -> None:
    harness = FakeHarness()
    harness.add_event("100", "code-writer", "urn:dec:dispatch:past")
    harness.add_event("101", "code-writer", "urn:dec:dispatch:future")
    client = _make_client(harness)

    async with EventDriver(
        worker_id="worker-warm",
        capability_tags=["code-writer"],
        endpoints=_make_endpoints(),
        client=client,
        retry_policy=_zero_policy(),
    ) as driver:
        # Pretend we already processed id=100 in a prior run.
        driver.set_last_event_id("100")
        seen: list[str] = []
        async for session in driver:
            seen.append(session.dispatch_id)
            await driver.complete(session.build_completion())
            break

    assert seen == ["urn:dec:dispatch:future"]
    assert harness.sse_connect_log[0]["last_event_id"] == "100"
