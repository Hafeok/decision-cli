"""Driver protocol abstracting production dispatch from replay dispatch (FT-083)."""

from __future__ import annotations

from types import TracebackType
from typing import Protocol, runtime_checkable

from ..session import Session
from ..types import CompletionPayload


@runtime_checkable
class Driver(Protocol):
    """Abstract source of `Session` objects for worker code.

    The `Driver` is the seam that lets the same worker body run unchanged in
    two modes:

    - Production (`EventDriver`, FT-084): a live SSE subscription against
      pipeline-cli, with atomic claims and completion POSTs over HTTP.
    - Replay (`ReplayDriver`, slice 2): an offline iterator over historical
      bundles, with completions captured locally for comparison rather than
      posted.

    Worker code consumes a driver only through this Protocol. Concretely:

        async with driver:
            async for session in driver:
                # ... fill the session with artifact / side-channel quads ...
                completion = session.build_completion()
                await driver.complete(completion)

    The Protocol is `runtime_checkable` so test scaffolding can assert
    `isinstance(obj, Driver)` for sanity, but production worker code MUST
    NOT branch on the concrete driver type. The whole point of the
    abstraction is that "which driver is in use" is not observable to the
    worker — `FakeDriver`, `EventDriver`, and (slice 2) `ReplayDriver` are
    interchangeable from the worker's perspective.

    ## What is observable to worker code

    - `Session` objects yielded by iteration. Each session carries its
      dispatch payload, identity (`session.id` = the harness's
      `prov:Activity` URI per ADR-050), and the in-memory bundle store.
    - The completion handoff via `await driver.complete(payload)`.
    - Lifecycle hooks via `__aenter__` / `__aexit__` (clean shutdown).

    ## What is NOT observable to worker code

    - Which subclass / implementation is producing the sessions.
    - Whether dispatches arrive over SSE or come from a pre-built list.
    - Whether completions are POSTed to a harness or stashed in memory.
    - Whether the driver runs against live state or a historical snapshot.
    """

    def __aiter__(self) -> "Driver":  # noqa: D401 — Protocol method
        """Return self as an async iterator of `Session` objects."""
        ...

    async def __anext__(self) -> Session:
        """Yield the next `Session`, or raise `StopAsyncIteration` when done."""
        ...

    async def complete(self, payload: CompletionPayload) -> None:
        """Hand a completion payload back through the driver.

        Production drivers POST it to the harness; replay drivers capture
        it for later comparison against an expected payload. Either way,
        worker code calls this exactly once per session and does not care
        what happens next.

        Raises whatever the concrete driver raises on terminal failure
        (e.g. `CompletionRejected` from the production poster). Callers
        treat any exception as a hard stop for that session.
        """
        ...

    async def aclose(self) -> None:
        """Release resources held by the driver.

        Idempotent — calling `aclose` more than once is a no-op. Once
        closed, the driver MUST stop yielding sessions (subsequent
        `__anext__` calls raise `StopAsyncIteration`).
        """
        ...

    async def __aenter__(self) -> "Driver":
        """Enter the driver's async context — opens any underlying transport."""
        ...

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> bool | None:
        """Exit the driver's async context — must call `aclose` for cleanup."""
        ...


__all__ = ["Driver"]
