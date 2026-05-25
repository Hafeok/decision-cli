"""TC-143: One dispatch → one completion lifecycle with in-memory pyoxigraph store.

Covers the four success criteria the parent feature_spec (FT-078) names:

1. A dispatch with N bundle triples produces a Session whose
   ``pyoxigraph.Store`` contains exactly those N triples on entry.
2. On clean completion, the completion payload contains: artifact
   triples, side-channel triples (if any), and the telemetry block.
3. On uncaught exception inside worker code, the SDK posts a `blocked`
   completion with the captured side-channel triples rather than dropping
   them.
4. The Session's URI is the same URI used by the harness's
   ``prov:Activity`` record for that dispatch (ADR-050) — verifiable by
   asserting ``session.id == completion.session_id ==`` the URI the
   harness handed in.

The harness wire surface is exercised by FT-077's TC-142; this test
focuses on the Session abstraction itself and its handoff to a
``CompletionPayload`` (which TC-142 already proves the wire can POST).
"""

from __future__ import annotations

import json

import pyoxigraph
import pytest

from pipeline_worker_sdk import (
    OUTCOME_BLOCKED,
    OUTCOME_ESCALATED,
    OUTCOME_FAILED,
    OUTCOME_SUCCESS,
    CompletionPayload,
    DispatchEvent,
    Session,
)


# --------------------------------------------------------------------------- #
# Bundle helpers — generate N-Quads payloads with a known triple count.        #
# --------------------------------------------------------------------------- #


def _bundle_nquads(count: int, graph: str = "http://example.org/g/bundle") -> str:
    """Produce ``count`` distinct N-Quads in a single named graph."""
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
    session_iri: str | None = None,
    dispatch_id: str = "urn:dec:dispatch:test",
    capability_tag: str = "code-writer",
) -> DispatchEvent:
    metadata: dict[str, str] = {"role_id": "implementer"}
    if session_iri is not None:
        metadata["session_id"] = session_iri
    return DispatchEvent(
        event_id="1",
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
# Criterion #1 — bundle ingest: N N-Quads in ⇒ N quads in the session store.   #
# --------------------------------------------------------------------------- #


@pytest.mark.parametrize("n", [0, 1, 5, 17])
def test_bundle_ingest_produces_store_of_expected_size(n: int) -> None:
    dispatch = _make_dispatch(nquads=_bundle_nquads(n))
    session = Session(dispatch)
    assert session.bundle_size == n
    assert len(session.store) == n
    # Idempotence: reading the store more than once does not mutate it.
    assert len(list(session.store)) == n


def test_bundle_store_is_queryable_via_sparql() -> None:
    nq = _bundle_nquads(3, graph="http://example.org/g/bundle")
    session = Session(_make_dispatch(nquads=nq))
    rows = list(
        session.store.query(
            "SELECT (COUNT(*) AS ?c) WHERE { GRAPH ?g { ?s ?p ?o } }"
        )
    )
    assert len(rows) == 1
    count_term = rows[0]["c"]
    # pyoxigraph returns a literal whose lexical value is the count.
    assert int(count_term.value) == 3


# --------------------------------------------------------------------------- #
# Criterion #2 — clean completion: artifact + side-channel + telemetry.        #
# --------------------------------------------------------------------------- #


def test_clean_completion_contains_artifact_side_channel_and_telemetry() -> None:
    bundle = _bundle_nquads(2)
    session = Session(
        _make_dispatch(nquads=bundle, session_iri="urn:dec:session:clean-1"),
        agent_iri="urn:dec:agent:implementer",
    )

    session.emit_artifact_nquads(
        "<http://example.org/artifact/1> "
        "<http://example.org/produced> "
        '"value-1" '
        "<http://example.org/g/artifact> ."
    )
    session.emit_side_channel_nquads(
        "<http://example.org/feedback/1> "
        "<http://example.org/feedbackClass> "
        '"defect" '
        "<http://example.org/g/side> ."
    )
    session.record_telemetry("model_group", "lgroup-code")
    session.accumulate("input_tokens", 1234)
    session.accumulate("output_tokens", 567)
    session.accumulate("input_tokens", 100)  # accumulate is additive

    payload = session.build_completion()

    assert isinstance(payload, CompletionPayload)
    assert payload.outcome == OUTCOME_SUCCESS
    assert payload.dispatch_id == "urn:dec:dispatch:test"
    assert payload.session_id == "urn:dec:session:clean-1"

    quads = _parse_nquads(payload.nquads_payload)
    assert len(quads) == 2
    subjects = {str(q.subject) for q in quads}
    assert "<http://example.org/artifact/1>" in subjects
    assert "<http://example.org/feedback/1>" in subjects

    # Telemetry: counter accumulation + scalar fields + structural counts.
    assert payload.telemetry["model_group"] == "lgroup-code"
    assert payload.telemetry["input_tokens"] == 1334
    assert payload.telemetry["output_tokens"] == 567
    assert payload.telemetry["bundle_quad_count"] == 2
    assert payload.telemetry["artifact_quad_count"] == 1
    assert payload.telemetry["side_channel_quad_count"] == 1
    assert payload.telemetry["session_id"] == "urn:dec:session:clean-1"
    assert payload.telemetry["dispatch_id"] == "urn:dec:dispatch:test"
    assert payload.telemetry["capability_tag"] == "code-writer"
    assert "started_at" in payload.telemetry
    assert "ended_at" in payload.telemetry


def test_clean_completion_with_no_emissions_yields_empty_payload() -> None:
    session = Session(_make_dispatch(nquads=_bundle_nquads(1)))
    payload = session.build_completion()
    assert payload.outcome == OUTCOME_SUCCESS
    assert payload.nquads_payload == ""
    assert payload.telemetry["artifact_quad_count"] == 0
    assert payload.telemetry["side_channel_quad_count"] == 0


def test_double_completion_is_a_programming_error() -> None:
    session = Session(_make_dispatch())
    session.build_completion()
    with pytest.raises(RuntimeError, match="already produced a completion"):
        session.build_completion()


# --------------------------------------------------------------------------- #
# Criterion #3 — blocked completion preserves side-channel triples.            #
# --------------------------------------------------------------------------- #


def test_blocked_completion_drops_artifact_keeps_side_channel() -> None:
    session = Session(
        _make_dispatch(session_iri="urn:dec:session:blocked-1"),
    )
    # The worker got far enough to emit one artifact triple AND one
    # side-channel triple before raising — the blocked path must preserve
    # the side-channel but not the half-formed artifact.
    session.emit_artifact_nquads(
        "<http://example.org/half> "
        "<http://example.org/produced> "
        '"partial" '
        "<http://example.org/g/artifact> ."
    )
    session.emit_side_channel_nquads(
        "<http://example.org/feedback/blocked> "
        "<http://example.org/feedbackClass> "
        '"unimplementable" '
        "<http://example.org/g/side> ."
    )

    payload = session.build_blocked_completion(error="bundle gap on row 42")

    assert payload.outcome == OUTCOME_BLOCKED
    assert payload.session_id == "urn:dec:session:blocked-1"
    quads = _parse_nquads(payload.nquads_payload)
    assert len(quads) == 1
    assert str(quads[0].subject) == "<http://example.org/feedback/blocked>"
    # Telemetry captures the error reason and the structural counts.
    assert payload.telemetry["error"] == "bundle gap on row 42"
    assert payload.telemetry["side_channel_quad_count"] == 1


def test_blocked_completion_with_no_side_channel_yields_empty_payload() -> None:
    session = Session(_make_dispatch())
    payload = session.build_blocked_completion(error="silent failure")
    assert payload.outcome == OUTCOME_BLOCKED
    assert payload.nquads_payload == ""
    assert payload.telemetry["error"] == "silent failure"


def test_blocked_completion_outcome_can_be_overridden() -> None:
    session = Session(_make_dispatch())
    payload = session.build_blocked_completion(
        error="downstream escalation requested",
        outcome=OUTCOME_ESCALATED,
    )
    assert payload.outcome == OUTCOME_ESCALATED


def test_context_manager_records_uncaught_exception_on_blocked_path() -> None:
    session_iri = "urn:dec:session:ctx-exc"
    dispatch = _make_dispatch(session_iri=session_iri)

    captured: list[CompletionPayload] = []

    class WorkerCrashed(RuntimeError):
        pass

    try:
        with Session(dispatch) as session:
            session.emit_side_channel_nquads(
                "<http://example.org/feedback/exc> "
                "<http://example.org/feedbackClass> "
                '"gap" '
                "<http://example.org/g/side> ."
            )
            raise WorkerCrashed("model timed out")
    except WorkerCrashed:
        captured.append(session.build_blocked_completion(error="model timed out"))

    assert len(captured) == 1
    payload = captured[0]
    assert payload.outcome == OUTCOME_BLOCKED
    # The context manager attached the traceback summary even before the
    # caller built the blocked payload — verifies no side-channel triple
    # was dropped on the exception path.
    assert payload.telemetry["error"] == "model timed out"
    assert "WorkerCrashed" in payload.telemetry.get("uncaught_exception", "")
    assert payload.telemetry["side_channel_quad_count"] == 1
    quads = _parse_nquads(payload.nquads_payload)
    assert len(quads) == 1


# --------------------------------------------------------------------------- #
# Criterion #4 — Session URI identity (ADR-050).                               #
# --------------------------------------------------------------------------- #


def test_session_uri_matches_harness_supplied_activity_iri() -> None:
    activity_iri = "urn:dec:session:activity-7777"
    session = Session(_make_dispatch(session_iri=activity_iri))
    assert session.id == activity_iri
    payload = session.build_completion()
    assert payload.session_id == activity_iri


def test_session_uri_falls_back_to_metadata_session_id() -> None:
    activity_iri = "urn:dec:session:metadata-only"
    session = Session(
        DispatchEvent(
            event_id="9",
            dispatch_id="urn:dec:dispatch:mdfallback",
            capability_tag="code-writer",
            nquads_payload="",
            metadata={"session_id": activity_iri},
        )
    )
    assert session.id == activity_iri


def test_session_uri_minted_locally_when_harness_supplies_none() -> None:
    session = Session(_make_dispatch())
    assert session.id.startswith("urn:dec:session:")
    # Two minted sessions should not collide.
    other = Session(_make_dispatch())
    assert session.id != other.id


def test_session_payload_round_trips_via_pydantic_dump() -> None:
    """The wire layer (FT-077) calls ``payload.model_dump(mode='json')``;
    the Session must produce a payload that survives that round-trip."""
    session = Session(
        _make_dispatch(
            nquads=_bundle_nquads(1),
            session_iri="urn:dec:session:roundtrip",
        )
    )
    session.emit_artifact_nquads(
        "<http://example.org/a> <http://example.org/p> "
        '"v" <http://example.org/g> .'
    )
    session.accumulate("input_tokens", 42)
    payload = session.build_completion()
    encoded = json.dumps(payload.model_dump(mode="json"))
    decoded = CompletionPayload.model_validate(json.loads(encoded))
    assert decoded.session_id == "urn:dec:session:roundtrip"
    assert decoded.outcome == OUTCOME_SUCCESS
    assert decoded.telemetry["input_tokens"] == 42
    assert len(_parse_nquads(decoded.nquads_payload)) == 1


# --------------------------------------------------------------------------- #
# Constant exports — make sure ADR-050 outcome vocabulary is exposed.          #
# --------------------------------------------------------------------------- #


def test_outcome_constants_exported() -> None:
    assert OUTCOME_SUCCESS == "success"
    assert OUTCOME_BLOCKED == "blocked"
    assert OUTCOME_ESCALATED == "escalated"
    assert OUTCOME_FAILED == "failed"
