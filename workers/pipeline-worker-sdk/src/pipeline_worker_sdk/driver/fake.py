"""In-memory FakeDriver test double conforming to the Driver protocol (FT-083)."""

from __future__ import annotations

from collections.abc import Iterable
from dataclasses import dataclass, field
from types import TracebackType

from ..session import Session
from ..types import CompletionPayload, DispatchEvent
from .base import Driver


@dataclass
class FakeDispatch:
    """One pre-built dispatch + optional expected-completion pairing.

    `bundle` is the `DispatchEvent` the FakeDriver hands to worker code on
    the next iteration step. `expected_completion`, if provided, is the
    completion payload the FakeDriver expects the worker to produce; it
    is NOT checked automatically by `complete()` (the SDK test suite
    decides what to assert), but the FakeDriver records every received
    completion so tests can compare against it.
    """

    bundle: DispatchEvent
    expected_completion: CompletionPayload | None = None


@dataclass
class FakeDriver:
    """Pre-built in-memory `Driver` for SDK unit tests.

    Construct with a list of `(bundle, expected_completion)` pairs (either
    raw `FakeDispatch` instances or 2-tuples). Iterating yields one
    `Session` per pair; `complete()` records the completion in
    `received_completions` so the test can assert on shape.

    Lifecycle:

    - Workers see exactly the sessions the test pre-built, in order.
    - `aclose()` is idempotent; further iteration raises `StopAsyncIteration`.
    - `__aenter__` / `__aexit__` work as normal async-context-manager hooks
      so worker code that uses `async with driver:` runs unchanged.

    Conforms to the `Driver` Protocol; an `isinstance(driver, Driver)`
    check passes. The whole point: worker code written against `Driver`
    runs under this fake in tests AND under `EventDriver` in production
    without changing.
    """

    dispatches: list[FakeDispatch] = field(default_factory=list)
    received_completions: list[CompletionPayload] = field(default_factory=list)
    _index: int = field(default=0, init=False)
    _closed: bool = field(default=False, init=False)
    _opened: bool = field(default=False, init=False)
    _yielded_sessions: list[Session] = field(default_factory=list, init=False)

    @classmethod
    def from_pairs(
        cls,
        pairs: Iterable[
            FakeDispatch
            | tuple[DispatchEvent, CompletionPayload | None]
            | tuple[DispatchEvent]
            | DispatchEvent
        ],
    ) -> "FakeDriver":
        """Construct from a mix of `FakeDispatch`, tuples, or bare `DispatchEvent`s."""
        normalized: list[FakeDispatch] = []
        for entry in pairs:
            if isinstance(entry, FakeDispatch):
                normalized.append(entry)
            elif isinstance(entry, DispatchEvent):
                normalized.append(FakeDispatch(bundle=entry))
            elif isinstance(entry, tuple):
                if len(entry) == 1:
                    normalized.append(FakeDispatch(bundle=entry[0]))
                elif len(entry) == 2:
                    normalized.append(
                        FakeDispatch(bundle=entry[0], expected_completion=entry[1])
                    )
                else:
                    raise ValueError(
                        f"FakeDriver pair must have 1 or 2 elements; got {len(entry)}"
                    )
            else:
                raise TypeError(
                    "FakeDriver pair must be FakeDispatch, DispatchEvent, or tuple"
                )
        return cls(dispatches=normalized)

    # ------------------------------------------------------------------ #
    # Driver protocol                                                    #
    # ------------------------------------------------------------------ #

    def __aiter__(self) -> "FakeDriver":
        return self

    async def __anext__(self) -> Session:
        if self._closed or self._index >= len(self.dispatches):
            raise StopAsyncIteration
        entry = self.dispatches[self._index]
        self._index += 1
        session = Session(entry.bundle)
        self._yielded_sessions.append(session)
        return session

    async def complete(self, payload: CompletionPayload) -> None:
        """Record the completion. No POST is issued; tests inspect the list."""
        if self._closed:
            raise RuntimeError("FakeDriver is closed; cannot accept completions")
        self.received_completions.append(payload)

    async def aclose(self) -> None:
        self._closed = True

    async def __aenter__(self) -> "FakeDriver":
        self._opened = True
        return self

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> bool | None:
        await self.aclose()
        return False

    # ------------------------------------------------------------------ #
    # Test-side conveniences (NOT part of the Driver protocol)           #
    # ------------------------------------------------------------------ #

    @property
    def closed(self) -> bool:
        """True iff `aclose` has been called (directly or via `__aexit__`)."""
        return self._closed

    @property
    def opened(self) -> bool:
        """True iff `__aenter__` has been called."""
        return self._opened

    @property
    def yielded_sessions(self) -> tuple[Session, ...]:
        """Snapshot of every Session this driver has yielded."""
        return tuple(self._yielded_sessions)

    @property
    def remaining(self) -> int:
        """How many dispatches have not yet been yielded."""
        return max(0, len(self.dispatches) - self._index)


# Static check: FakeDriver satisfies the Driver Protocol.
_protocol_witness: Driver = FakeDriver()


__all__ = ["FakeDispatch", "FakeDriver"]
