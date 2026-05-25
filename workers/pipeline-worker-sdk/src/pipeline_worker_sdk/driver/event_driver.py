"""Production Driver wiring SSE+POST wire layer to the Session lifecycle (FT-084)."""

from __future__ import annotations

import asyncio
import logging
from collections.abc import AsyncIterator, Iterable
from types import TracebackType

import httpx

from ..poster import CompletionFailed, CompletionRejected, RetryPolicy
from ..session import OUTCOME_BLOCKED, Session
from ..types import CompletionPayload, DispatchEvent
from ..wire import HarnessEndpoints, WireClient

_LOG = logging.getLogger(__name__)


class EventDriver:
    """Concrete `Driver` wiring SSE+POST wire (FT-077) to Session lifecycle (FT-078).

    One instance = one worker process: it advertises a fixed set of
    capability tags, walks the SSE stream (resuming via ``Last-Event-ID``
    on transient disconnects per ADR-045), claims each matching dispatch
    atomically, hands the Session (whose URI IS the harness's
    ``prov:Activity`` URI per ADR-050) to worker code via
    ``async for session in driver:``, then POSTs the worker's resulting
    completion through the wire layer.

    Failure surfacing:

    - **Lost claim**: skip the dispatch silently; the SDK pulls the next
      SSE envelope. No session is yielded.
    - **Transient SSE disconnect**: caught by ``_next_dispatch`` and
      retried with bounded backoff. The wire layer's resume cursor
      survives reconnects so the harness only replays strictly-newer
      events.
    - **Transient completion POST failure**: retried by `CompletionPoster`
      per `RetryPolicy`. Exhausting retries raises `CompletionFailed`
      out of ``driver.complete(...)``.
    - **Deterministic 4xx rejection**: `CompletionRejected` propagates
      identically; the worker treats it as the dispatch's terminal result.

    The driver does NOT auto-build completions on the worker's behalf —
    worker code calls ``session.build_completion()`` then
    ``await driver.complete(payload)``. Symmetric with FakeDriver
    (FT-083) so the same worker body runs unchanged.
    """

    def __init__(
        self,
        worker_id: str,
        capability_tags: Iterable[str],
        endpoints: HarnessEndpoints,
        *,
        client: httpx.AsyncClient | None = None,
        retry_policy: RetryPolicy | None = None,
        catalog_ttl_seconds: float = 300.0,
        sse_reconnect_policy: RetryPolicy | None = None,
        sleep_fn=asyncio.sleep,
    ) -> None:
        self._wire = WireClient(
            worker_id=worker_id,
            capability_tags=capability_tags,
            endpoints=endpoints,
            client=client,
            retry_policy=retry_policy,
            catalog_ttl_seconds=catalog_ttl_seconds,
        )
        # SSE reconnect policy is separate from the completion-post policy:
        # SSE drops are expected on a long-lived stream; completion POSTs
        # are short RPCs. They share the RetryPolicy shape but tune the
        # backoff differently in production.
        self._sse_reconnect_policy = sse_reconnect_policy or RetryPolicy(
            max_attempts=8, base_backoff=0.5, max_backoff=30.0
        )
        self._sleep = sleep_fn
        self._closed = False
        self._opened = False
        self._dispatch_iter: AsyncIterator[DispatchEvent] | None = None
        # The session most recently yielded to worker code, retained so
        # ``__aexit__`` can best-effort post a blocked completion if the
        # worker crashed before calling ``complete``.
        self._active_session: Session | None = None

    # ------------------------------------------------------------------ #
    # Identity / introspection (NOT part of the Driver protocol)         #
    # ------------------------------------------------------------------ #

    @property
    def worker_id(self) -> str:
        return self._wire.worker_id

    @property
    def capability_tags(self) -> frozenset[str]:
        return self._wire.capability_tags

    @property
    def wire(self) -> WireClient:
        """Underlying WireClient — exposed for tests and operational tools."""
        return self._wire

    @property
    def last_event_id(self) -> str | None:
        return self._wire.last_event_id

    def set_last_event_id(self, event_id: str | None) -> None:
        """Warm-start the SSE resume cursor from a persisted checkpoint."""
        self._wire.set_last_event_id(event_id)

    @property
    def closed(self) -> bool:
        return self._closed

    @property
    def opened(self) -> bool:
        return self._opened

    # ------------------------------------------------------------------ #
    # Driver protocol — async iteration over Session objects             #
    # ------------------------------------------------------------------ #

    def __aiter__(self) -> "EventDriver":
        return self

    async def __anext__(self) -> Session:
        if self._closed:
            raise StopAsyncIteration
        while True:
            dispatch = await self._next_dispatch()
            if dispatch is None:
                raise StopAsyncIteration
            if not await self._safe_claim(dispatch):
                # Lost the claim race or hit a network blip — skip and
                # wait for the next dispatch envelope.
                continue
            session = Session(dispatch)
            self._active_session = session
            return session

    async def complete(self, payload: CompletionPayload) -> None:
        """POST the completion through the wire layer.

        Raises `CompletionFailed` once retries are exhausted on transient
        failures, or `CompletionRejected` on a 4xx with the harness's
        validation report.
        """
        if self._closed:
            raise RuntimeError(
                f"EventDriver({self.worker_id}) is closed; cannot accept completions"
            )
        try:
            await self._wire.complete(payload)
        finally:
            # The session is terminal once a completion is attempted —
            # whether the POST succeeded or raised, clear the marker so
            # ``__aexit__`` does not also try to post a blocked one.
            self._active_session = None

    async def aclose(self) -> None:
        if self._closed:
            return
        self._closed = True
        self._dispatch_iter = None
        self._active_session = None
        await self._wire.aclose()

    async def __aenter__(self) -> "EventDriver":
        self._opened = True
        return self

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> bool | None:
        # If a session was yielded but the worker raised before producing
        # a completion, post a blocked completion so the harness sees the
        # session terminate cleanly instead of hanging.
        session = self._active_session
        if session is not None and exc is not None and not session.closed:
            await self._best_effort_blocked_completion(session, exc)
        await self.aclose()
        return False

    # ------------------------------------------------------------------ #
    # Internal: SSE dispatch loop with bounded reconnect.                #
    # ------------------------------------------------------------------ #

    async def _next_dispatch(self) -> DispatchEvent | None:
        """Yield the next ``DispatchEvent`` from the SSE stream.

        On transient disconnect, reconnects up to the SSE reconnect
        policy's attempt count, carrying the ``Last-Event-ID`` cursor so
        the harness only replays strictly-newer events. Returns ``None``
        once the stream ends cleanly with no more events; raises the
        last transport exception if reconnects are exhausted.
        """
        policy = self._sse_reconnect_policy
        retry_attempts = 0
        end_of_stream_attempts = 0
        while True:
            if self._dispatch_iter is None:
                self._dispatch_iter = self._wire.dispatches().__aiter__()
            try:
                return await self._dispatch_iter.__anext__()
            except StopAsyncIteration:
                # The harness closed the stream with no further events.
                # Try one reconnect: a clean close mid-stream is normal
                # for long-lived SSE; if the next connect also produces
                # no events, treat that as end-of-stream.
                self._dispatch_iter = None
                if end_of_stream_attempts >= 1:
                    return None
                end_of_stream_attempts += 1
                continue
            except httpx.HTTPError as exc:
                _LOG.warning(
                    "EventDriver(%s) SSE disconnect: %s: %s",
                    self.worker_id,
                    type(exc).__name__,
                    exc,
                )
                self._dispatch_iter = None
                retry_attempts += 1
                if retry_attempts >= policy.max_attempts:
                    raise
                await self._sleep(policy.sleep_for(retry_attempts))

    async def _safe_claim(self, dispatch: DispatchEvent) -> bool:
        """Atomically claim the dispatch; return True iff this worker won."""
        try:
            result = await self._wire.claim(dispatch.dispatch_id)
        except httpx.HTTPError as exc:
            # Treat unexpected transport errors as a lost claim — never
            # run a dispatch we could not confirm we own.
            _LOG.warning(
                "EventDriver(%s) claim error on %s: %s",
                self.worker_id,
                dispatch.dispatch_id,
                exc,
            )
            return False
        if not result.won:
            _LOG.debug(
                "EventDriver(%s) lost claim on %s (%s)",
                self.worker_id,
                dispatch.dispatch_id,
                result.reason,
            )
            return False
        return True

    async def _best_effort_blocked_completion(
        self, session: Session, exc: BaseException
    ) -> None:
        """POST a blocked completion when the worker crashed mid-session.

        Best-effort: any failure here is logged but not re-raised — the
        original exception from the worker body is the one the caller
        cares about.
        """
        try:
            payload = session.build_blocked_completion(
                error=f"worker raised {type(exc).__name__}: {exc}",
                outcome=OUTCOME_BLOCKED,
            )
        except RuntimeError as build_exc:
            # ``build_blocked_completion`` raises if the session is
            # already closed — that's fine, the worker produced its own
            # completion before raising.
            _LOG.debug(
                "EventDriver(%s) skipped blocked completion for %s: %s",
                self.worker_id,
                session.id,
                build_exc,
            )
            return
        try:
            await self._wire.complete(payload)
        except (CompletionFailed, CompletionRejected, httpx.HTTPError) as wire_exc:
            _LOG.error(
                "EventDriver(%s) failed to post blocked completion for %s: %s",
                self.worker_id,
                session.id,
                wire_exc,
            )


__all__ = ["EventDriver"]
