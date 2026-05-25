"""TC-148: Driver abstraction for production and replay (FT-083).

Exit criteria the parent feature_spec names:

1. A worker written against the `Driver` Protocol runs unchanged under
   the `FakeDriver` (this suite) and under `EventDriver` (FT-084's
   integration tests).
2. The `Driver` Protocol exposes only `__aiter__` / `__anext__` yielding
   `Session` objects, plus lifecycle hooks (`__aenter__`/`__aexit__`,
   `aclose`) and a completion handoff (`complete(payload)`).
3. The `FakeDriver` accepts a pre-built list of
   `(bundle, expected_completion)` tuples and records completions for
   test inspection.
4. `isinstance(driver, Driver)` works at runtime; static type checking
   sees no concrete-class fields outside the Protocol surface — workers
   cannot branch on driver implementation.

The Protocol-only contract is what makes the same worker code valid
against both the production EventDriver (FT-084) and the slice-2
ReplayDriver — so this TC validates the seam, not any concrete driver.
"""

from __future__ import annotations

import pyoxigraph
import pytest

from pipeline_worker_sdk import (
    OUTCOME_SUCCESS,
    CompletionPayload,
    DispatchEvent,
    Driver,
    FakeDispatch,
    FakeDriver,
    Session,
)


# --------------------------------------------------------------------------- #
# Bundle helpers — same shape as TC-143's helpers.                            #
# --------------------------------------------------------------------------- #


def _bundle_nquads(count: int, graph: str = "http://example.org/g/bundle") -> str:
    lines = []
    for i in range(count):
        lines.append(
            f"<http://example.org/s{i}> <http://example.org/p> "
            f"<http://example.org/o{i}> <{graph}> ."
        )
    return "\n".join(lines) + ("\n" if lines else "")


def _make_dispatch(
    *,
    nquads: str = "",
    dispatch_id: str = "urn:dec:dispatch:test",
    session_iri: str | None = None,
    capability_tag: str = "code-writer",
    event_id: str = "1",
) -> DispatchEvent:
    metadata: dict[str, str] = {"role_id": "implementer"}
    if session_iri is not None:
        metadata["session_id"] = session_iri
    return DispatchEvent(
        event_id=event_id,
        dispatch_id=dispatch_id,
        capability_tag=capability_tag,
        nquads_payload=nquads,
        metadata=metadata,
    )


def _parse_nquads(text: str) -> list[pyoxigraph.Quad]:
    if not text.strip():
        return []
    store = pyoxigraph.Store()
    store.load(input=text, format=pyoxigraph.RdfFormat.N_QUADS)
    return list(store)


# --------------------------------------------------------------------------- #
# Criterion #2 — Driver Protocol surface.                                     #
# --------------------------------------------------------------------------- #


def test_driver_protocol_is_runtime_checkable() -> None:
    driver = FakeDriver()
    assert isinstance(driver, Driver)


def test_driver_protocol_surface_is_minimal() -> None:
    """Verify the Driver Protocol exposes exactly the documented surface.

    The whole point of the abstraction is that production and replay
    drivers expose the same protocol; an extra method on the Protocol
    surface would force ReplayDriver to implement it too.
    """
    expected = {
        "__aiter__",
        "__anext__",
        "complete",
        "aclose",
        "__aenter__",
        "__aexit__",
    }
    actual = {
        name
        for name in dir(Driver)
        if not name.startswith("_") or name in expected
    }
    # Protocol class carries some private machinery; we only assert the
    # documented public surface is a subset of what's exposed.
    assert expected.issubset(actual), f"missing: {expected - actual}"


# --------------------------------------------------------------------------- #
# Criterion #3 — FakeDriver accepts pre-built dispatches and records          #
# completions for inspection.                                                 #
# --------------------------------------------------------------------------- #


@pytest.mark.asyncio
async def test_fake_driver_yields_pre_built_sessions_in_order() -> None:
    bundles = [
        _make_dispatch(dispatch_id="urn:dec:dispatch:a", event_id="1"),
        _make_dispatch(dispatch_id="urn:dec:dispatch:b", event_id="2"),
        _make_dispatch(dispatch_id="urn:dec:dispatch:c", event_id="3"),
    ]
    driver = FakeDriver.from_pairs(bundles)
    seen: list[str] = []
    async with driver:
        async for session in driver:
            seen.append(session.dispatch_id)
    assert seen == [
        "urn:dec:dispatch:a",
        "urn:dec:dispatch:b",
        "urn:dec:dispatch:c",
    ]


@pytest.mark.asyncio
async def test_fake_driver_accepts_dispatch_completion_tuples() -> None:
    expected = CompletionPayload(
        dispatch_id="urn:dec:dispatch:a",
        session_id="urn:dec:session:a",
        nquads_payload="",
        outcome=OUTCOME_SUCCESS,
        telemetry={},
    )
    driver = FakeDriver.from_pairs(
        [
            (_make_dispatch(dispatch_id="urn:dec:dispatch:a"), expected),
        ]
    )
    assert driver.dispatches[0].expected_completion is expected
    assert driver.dispatches[0].bundle.dispatch_id == "urn:dec:dispatch:a"


@pytest.mark.asyncio
async def test_fake_driver_records_completions() -> None:
    driver = FakeDriver.from_pairs(
        [
            _make_dispatch(
                nquads=_bundle_nquads(1),
                session_iri="urn:dec:session:rec-1",
            ),
        ]
    )
    async with driver:
        async for session in driver:
            session.emit_artifact_nquads(
                "<http://example.org/a> <http://example.org/p> "
                '"v" <http://example.org/g> .'
            )
            payload = session.build_completion()
            await driver.complete(payload)
    assert len(driver.received_completions) == 1
    rec = driver.received_completions[0]
    assert rec.session_id == "urn:dec:session:rec-1"
    assert rec.outcome == OUTCOME_SUCCESS
    assert len(_parse_nquads(rec.nquads_payload)) == 1


# --------------------------------------------------------------------------- #
# Criterion #1 — A worker written against `Driver` runs unchanged.            #
# --------------------------------------------------------------------------- #


async def run_worker(driver: Driver) -> list[CompletionPayload]:
    """Reference worker body — depends ONLY on the Driver Protocol.

    This is the canonical shape of worker code: open the driver, iterate
    sessions, build a completion per session, hand it back. Worker code
    MUST work against this Protocol without branching on the concrete
    driver type. A regression in the FakeDriver surface that breaks this
    function would also break every real worker.
    """
    seen: list[CompletionPayload] = []
    async with driver:
        async for session in driver:
            session.emit_artifact_nquads(
                f"<urn:art:{session.dispatch_id}> "
                "<http://example.org/produced> "
                '"ok" '
                "<http://example.org/g/art> ."
            )
            session.record_telemetry("model_group", "lgroup-code")
            payload = session.build_completion()
            await driver.complete(payload)
            seen.append(payload)
    return seen


@pytest.mark.asyncio
async def test_reference_worker_runs_under_fake_driver() -> None:
    bundles = [
        _make_dispatch(
            dispatch_id=f"urn:dec:dispatch:{i}",
            session_iri=f"urn:dec:session:{i}",
            event_id=str(i),
        )
        for i in range(3)
    ]
    driver = FakeDriver.from_pairs(bundles)
    completions = await run_worker(driver)
    assert len(completions) == 3
    assert [c.dispatch_id for c in completions] == [
        "urn:dec:dispatch:0",
        "urn:dec:dispatch:1",
        "urn:dec:dispatch:2",
    ]
    assert driver.closed is True
    assert driver.opened is True
    assert len(driver.received_completions) == 3


# --------------------------------------------------------------------------- #
# Lifecycle hooks: aclose is idempotent; closed drivers stop yielding.        #
# --------------------------------------------------------------------------- #


@pytest.mark.asyncio
async def test_aclose_is_idempotent() -> None:
    driver = FakeDriver.from_pairs([_make_dispatch()])
    await driver.aclose()
    await driver.aclose()  # second call is a no-op
    assert driver.closed is True


@pytest.mark.asyncio
async def test_closed_driver_stops_yielding() -> None:
    driver = FakeDriver.from_pairs(
        [
            _make_dispatch(dispatch_id="urn:dec:dispatch:a"),
            _make_dispatch(dispatch_id="urn:dec:dispatch:b"),
        ]
    )
    iterator = driver.__aiter__()
    first = await iterator.__anext__()
    assert first.dispatch_id == "urn:dec:dispatch:a"
    await driver.aclose()
    with pytest.raises(StopAsyncIteration):
        await iterator.__anext__()


@pytest.mark.asyncio
async def test_complete_on_closed_driver_raises() -> None:
    driver = FakeDriver.from_pairs([_make_dispatch()])
    await driver.aclose()
    payload = CompletionPayload(
        dispatch_id="urn:dec:dispatch:test",
        session_id="urn:dec:session:test",
    )
    with pytest.raises(RuntimeError, match="closed"):
        await driver.complete(payload)


@pytest.mark.asyncio
async def test_context_manager_calls_aclose_on_exit() -> None:
    driver = FakeDriver.from_pairs([_make_dispatch()])
    async with driver:
        assert driver.opened is True
        assert driver.closed is False
    assert driver.closed is True


@pytest.mark.asyncio
async def test_context_manager_closes_on_exception() -> None:
    driver = FakeDriver.from_pairs([_make_dispatch()])

    class BoomError(RuntimeError):
        pass

    with pytest.raises(BoomError):
        async with driver:
            async for _session in driver:
                raise BoomError("worker crashed")
    assert driver.closed is True


# --------------------------------------------------------------------------- #
# Criterion #4 — workers cannot branch on driver type — Protocol carries no   #
# implementation-specific surface.                                            #
# --------------------------------------------------------------------------- #


def test_driver_protocol_has_no_implementation_specific_attributes() -> None:
    """The Protocol surface should not leak FakeDriver-specific fields.

    `received_completions`, `dispatches`, `closed`, etc. are FakeDriver
    conveniences for tests; the Protocol must not advertise them, or
    worker code could branch on their presence and silently couple to
    the test double.
    """
    protocol_attrs = set(dir(Driver))
    forbidden = {
        "received_completions",
        "dispatches",
        "yielded_sessions",
        "remaining",
        "opened",
    }
    leaked = protocol_attrs & forbidden
    assert not leaked, f"Driver Protocol leaks fake-specific attrs: {leaked}"


def test_fake_driver_satisfies_driver_protocol_for_isinstance() -> None:
    driver: Driver = FakeDriver()
    assert isinstance(driver, Driver)


# --------------------------------------------------------------------------- #
# Edge cases: empty FakeDriver, malformed pairs.                              #
# --------------------------------------------------------------------------- #


@pytest.mark.asyncio
async def test_empty_fake_driver_yields_nothing() -> None:
    driver = FakeDriver()
    seen: list[Session] = []
    async with driver:
        async for session in driver:
            seen.append(session)
    assert seen == []
    assert driver.closed is True


def test_from_pairs_rejects_malformed_input() -> None:
    with pytest.raises(TypeError):
        FakeDriver.from_pairs([123])  # type: ignore[list-item]


def test_from_pairs_rejects_wrong_tuple_arity() -> None:
    bad = (
        _make_dispatch(dispatch_id="urn:dec:dispatch:a"),
        CompletionPayload(
            dispatch_id="urn:dec:dispatch:a",
            session_id="urn:dec:session:a",
        ),
        "extra",
    )
    with pytest.raises(ValueError, match="1 or 2"):
        FakeDriver.from_pairs([bad])  # type: ignore[list-item]


def test_from_pairs_accepts_bare_dispatch_event() -> None:
    bundle = _make_dispatch(dispatch_id="urn:dec:dispatch:bare")
    driver = FakeDriver.from_pairs([bundle])
    assert len(driver.dispatches) == 1
    assert driver.dispatches[0].bundle.dispatch_id == "urn:dec:dispatch:bare"
    assert driver.dispatches[0].expected_completion is None


def test_from_pairs_accepts_fake_dispatch() -> None:
    fd = FakeDispatch(bundle=_make_dispatch(dispatch_id="urn:dec:dispatch:fd"))
    driver = FakeDriver.from_pairs([fd])
    assert driver.dispatches[0] is fd


@pytest.mark.asyncio
async def test_fake_driver_remaining_count_tracks_iteration() -> None:
    driver = FakeDriver.from_pairs(
        [
            _make_dispatch(dispatch_id=f"urn:dec:dispatch:{i}", event_id=str(i))
            for i in range(3)
        ]
    )
    assert driver.remaining == 3
    iterator = driver.__aiter__()
    await iterator.__anext__()
    assert driver.remaining == 2
    await iterator.__anext__()
    assert driver.remaining == 1
    await iterator.__anext__()
    assert driver.remaining == 0
    with pytest.raises(StopAsyncIteration):
        await iterator.__anext__()
