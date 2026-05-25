"""TC-147: pipeline-worker SDK side-channel — emergent judgments + feedback emission.

Covers the three success criteria FT-082 names:

1. ``session.record_emergent_judgment(decision, rationale)`` produces
   triples visible in the paired interpretation session's bundle. The
   judgment triples ride on the artifact emission set, so the harness
   ships them alongside the produced artifact and a downstream verifier
   can reach them via SPARQL on the bundle store.

2. ``session.emit_feedback(class=..., blocking=True, ...)`` causes the
   session to terminate with ``outcome=blocked`` (per ADR-025) and
   includes the Feedback artifact in the completion payload.

3. ``session.emit_feedback(class=..., blocking=False, ...)`` does NOT
   affect the session outcome (``success``) but produces a Feedback
   artifact in the completion payload alongside the main artifact.

All emissions go through the same wire transport (no separate channel);
the harness's GraphWriter chokepoint (ADR-041) validates them via
SHACL — this test only proves the worker-side surface.
"""

from __future__ import annotations

import pyoxigraph
import pytest

from pipeline_worker_sdk import (
    OUTCOME_BLOCKED,
    OUTCOME_SUCCESS,
    CompletionPayload,
    DispatchEvent,
    FeedbackEmission,
    Session,
)
from pipeline_worker_sdk.side_channel import (
    FEEDBACK_CLASSES,
    CLASS_BLOCKING_DEFAULTS,
    CLASS_TARGET_ROLE_DEFAULTS,
    build_feedback_quads,
    build_judgment_quads,
    emission_is_blocking,
    emission_target_role,
)
from pipeline_worker_sdk.side_channel.vocab import (
    DEC_DECISION,
    DEC_EMERGENT_JUDGMENT,
    DEC_EVIDENCE,
    DEC_FEEDBACK,
    DEC_FEEDBACK_CLASS,
    DEC_LIFECYCLE_STATE,
    DEC_RATIONALE,
    DEC_RECOMMENDATION,
    DEC_SOURCE_SESSION,
    DEC_TARGET_ROLE,
    FEEDBACK_STATE_PRODUCED,
)


# --------------------------------------------------------------------------- #
# Test scaffolding                                                            #
# --------------------------------------------------------------------------- #


def _make_dispatch(
    *,
    session_iri: str | None = None,
    dispatch_id: str = "urn:dec:dispatch:tc-147",
    capability_tag: str = "code-writer",
    nquads: str = "",
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


def _parse_nquads(text: str) -> pyoxigraph.Store:
    store = pyoxigraph.Store()
    if text.strip():
        store.load(input=text, format=pyoxigraph.RdfFormat.N_QUADS)
    return store


def _evidence(extra: str = "") -> str:
    """A bundle citation that comfortably clears the 20-char minimum."""
    base = "feature_spec line 42 is silent on the resume window"
    return f"{base} {extra}".strip()


# --------------------------------------------------------------------------- #
# Criterion #1 — emergent judgment lands on the artifact emission set         #
# --------------------------------------------------------------------------- #


def test_emergent_judgment_emits_quads_into_artifact_set() -> None:
    session = Session(_make_dispatch(session_iri="urn:dec:session:judg-1"))
    assert session.artifact_size == 0

    iri = session.record_emergent_judgment(
        decision="rename helper to compute_window_duration",
        rationale=(
            "in-authority naming change; the spec leaves helper names "
            "implementation-defined within the feature module."
        ),
    )

    # Recorded on the session and reachable via the public read surface.
    assert iri.startswith("urn:dec:emergent-judgment:")
    assert iri in session.emitted_judgment_iris
    # Five required predicates per build_judgment_quads.
    assert session.artifact_size == 5
    # Side-channel must NOT carry the judgment — it's metadata on the
    # produced artifact, not a feedback signal.
    assert session.side_channel_size == 0


def test_emergent_judgment_quads_surface_to_interpretation_bundle() -> None:
    """A worker emits a judgment; the completion payload ships it so the
    paired interpretation session sees it via the same bundle assembly
    path the harness uses to surface the produced artifact."""
    session = Session(_make_dispatch(session_iri="urn:dec:session:judg-2"))
    judgment_iri = session.record_emergent_judgment(
        decision="ship without retry helper",
        rationale=(
            "retry semantics are owned by FT-077; this feature's authority "
            "stops at the side-channel emission surface."
        ),
    )
    # The worker also produces its main artifact.
    session.emit_artifact_nquads(
        "<http://example.org/artifact/main> "
        "<http://example.org/produced> "
        '"value-main" '
        "<http://example.org/g/artifact> ."
    )

    payload = session.build_completion()
    assert payload.outcome == OUTCOME_SUCCESS

    bundle = _parse_nquads(payload.nquads_payload)
    judgment_subject = pyoxigraph.NamedNode(judgment_iri)
    rows = list(
        bundle.quads_for_pattern(
            judgment_subject,
            pyoxigraph.NamedNode(DEC_DECISION),
            None,
            None,
        )
    )
    assert len(rows) == 1
    assert "ship without retry helper" in str(rows[0].object)

    # The rationale and source-session predicates must also be reachable.
    rationale_rows = list(
        bundle.quads_for_pattern(
            judgment_subject,
            pyoxigraph.NamedNode(DEC_RATIONALE),
            None,
            None,
        )
    )
    assert len(rationale_rows) == 1
    sess_rows = list(
        bundle.quads_for_pattern(
            judgment_subject,
            pyoxigraph.NamedNode(DEC_SOURCE_SESSION),
            None,
            None,
        )
    )
    assert str(sess_rows[0].object) == "<urn:dec:session:judg-2>"

    # The rdf:type triple ties the artifact to dec:EmergentJudgment.
    type_rows = list(
        bundle.quads_for_pattern(
            judgment_subject,
            pyoxigraph.NamedNode(
                "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
            ),
            None,
            None,
        )
    )
    assert any(str(r.object) == f"<{DEC_EMERGENT_JUDGMENT}>" for r in type_rows)


def test_emergent_judgment_rejects_blank_inputs() -> None:
    session = Session(_make_dispatch())
    with pytest.raises(ValueError, match="decision"):
        session.record_emergent_judgment(decision="   ", rationale="ok")
    with pytest.raises(ValueError, match="rationale"):
        session.record_emergent_judgment(decision="ok", rationale="\t")


def test_emergent_judgment_refused_after_completion() -> None:
    session = Session(_make_dispatch())
    session.build_completion()
    with pytest.raises(RuntimeError, match="already produced a completion"):
        session.record_emergent_judgment(
            decision="late call", rationale="should not be allowed"
        )


# --------------------------------------------------------------------------- #
# Criterion #2 — blocking feedback ends the session with outcome=blocked      #
# --------------------------------------------------------------------------- #


def test_emit_feedback_blocking_true_yields_blocked_outcome() -> None:
    session = Session(_make_dispatch(session_iri="urn:dec:session:blk-1"))
    # Worker had time to start producing an artifact before noticing the
    # blocking issue — those triples must NOT make it into the payload.
    session.emit_artifact_nquads(
        "<http://example.org/artifact/half> "
        "<http://example.org/produced> "
        '"partial" '
        "<http://example.org/g/artifact> ."
    )

    feedback_iri = session.emit_feedback(
        feedback_class="gap",
        evidence=_evidence("retry policy is unspecified"),
        recommendation="Add an explicit retry policy section to the spec.",
        blocking=True,
    )

    assert session.has_blocking_feedback is True
    assert feedback_iri in session.emitted_feedback_iris

    payload = session.build_completion()
    assert payload.outcome == OUTCOME_BLOCKED

    bundle = _parse_nquads(payload.nquads_payload)
    feedback_subject = pyoxigraph.NamedNode(feedback_iri)
    # The Feedback artifact IS in the payload …
    class_rows = list(
        bundle.quads_for_pattern(
            feedback_subject,
            pyoxigraph.NamedNode(DEC_FEEDBACK_CLASS),
            None,
            None,
        )
    )
    assert len(class_rows) == 1
    assert "gap" in str(class_rows[0].object)
    # … but the half-formed artifact MUST be dropped.
    half_rows = list(
        bundle.quads_for_pattern(
            pyoxigraph.NamedNode("http://example.org/artifact/half"),
            None, None, None,
        )
    )
    assert half_rows == []


def test_emit_feedback_default_blocking_class_blocks_outcome() -> None:
    """The class default for `gap` is blocking (ADR-023 + ADR-025); a worker
    that omits `blocking=...` should still produce outcome=blocked."""
    session = Session(_make_dispatch())
    session.emit_feedback(
        feedback_class="contradiction",
        evidence=_evidence("ADR-022 vs ADR-026 disagree on routing"),
    )
    assert session.has_blocking_feedback is True
    payload = session.build_completion()
    assert payload.outcome == OUTCOME_BLOCKED


def test_blocking_feedback_overrides_explicit_success_request() -> None:
    """ADR-025: blocking feedback is load-bearing. Even if the worker
    asks for ``outcome=success`` (e.g. confused recovery code), the
    session refuses and lands at ``blocked``."""
    session = Session(_make_dispatch())
    session.emit_feedback(
        feedback_class="unimplementable",
        evidence=_evidence("tool surface insufficient for this task"),
    )
    payload = session.build_completion(outcome=OUTCOME_SUCCESS)
    assert payload.outcome == OUTCOME_BLOCKED


# --------------------------------------------------------------------------- #
# Criterion #3 — non-blocking feedback flows alongside outcome=success        #
# --------------------------------------------------------------------------- #


def test_emit_feedback_non_blocking_does_not_change_outcome() -> None:
    session = Session(_make_dispatch(session_iri="urn:dec:session:nb-1"))
    session.emit_artifact_nquads(
        "<http://example.org/artifact/main> "
        "<http://example.org/produced> "
        '"shipped-value" '
        "<http://example.org/g/artifact> ."
    )
    feedback_iri = session.emit_feedback(
        feedback_class="defect",
        evidence=_evidence("noticed an off-by-one on retry window length"),
        blocking=False,
    )

    assert session.has_blocking_feedback is False
    payload = session.build_completion()
    assert payload.outcome == OUTCOME_SUCCESS

    bundle = _parse_nquads(payload.nquads_payload)
    # Both the main artifact AND the Feedback artifact ride on the same
    # completion payload (one transport, no separate side-channel wire).
    main_rows = list(
        bundle.quads_for_pattern(
            pyoxigraph.NamedNode("http://example.org/artifact/main"),
            None, None, None,
        )
    )
    assert len(main_rows) == 1
    feedback_rows = list(
        bundle.quads_for_pattern(
            pyoxigraph.NamedNode(feedback_iri),
            pyoxigraph.NamedNode(DEC_FEEDBACK_CLASS),
            None, None,
        )
    )
    assert len(feedback_rows) == 1
    assert "defect" in str(feedback_rows[0].object)


def test_non_blocking_feedback_records_disposition_override_only_when_diverging() -> None:
    """ADR-025: ``dec:dispositionOverride`` is recorded only when the
    worker's blocking choice diverges from the class default."""
    # `gap` defaults to blocking — explicitly setting blocking=False
    # MUST record a disposition_override.
    session = Session(_make_dispatch(session_iri="urn:dec:session:override"))
    iri = session.emit_feedback(
        feedback_class="gap",
        evidence=_evidence("spec gap but action proceeded with assumption"),
        blocking=False,
        disposition_rationale="The gap concerns a non-critical edge case.",
    )
    payload = session.build_completion()
    bundle = _parse_nquads(payload.nquads_payload)
    override_rows = list(
        bundle.quads_for_pattern(
            pyoxigraph.NamedNode(iri),
            pyoxigraph.NamedNode(
                "https://decision-cli.dev/ns#dispositionOverride"
            ),
            None, None,
        )
    )
    assert len(override_rows) == 1
    assert "non-blocking" in str(override_rows[0].object)


def test_multiple_feedback_emissions_can_coexist() -> None:
    """A non-blocking emission followed by a blocking emission still
    leaves the session in the blocked path — blocking wins."""
    session = Session(_make_dispatch())
    iri_a = session.emit_feedback(
        feedback_class="defect",
        evidence=_evidence("non-blocking defect noted"),
    )
    iri_b = session.emit_feedback(
        feedback_class="gap",
        evidence=_evidence("blocking gap discovered after the defect"),
    )
    assert session.emitted_feedback_iris == (iri_a, iri_b)
    assert session.has_blocking_feedback is True
    payload = session.build_completion()
    assert payload.outcome == OUTCOME_BLOCKED
    # Both Feedback artifacts must be present in the payload, since
    # side-channel triples are preserved on the blocked path.
    bundle = _parse_nquads(payload.nquads_payload)
    for iri in (iri_a, iri_b):
        rows = list(
            bundle.quads_for_pattern(
                pyoxigraph.NamedNode(iri),
                pyoxigraph.NamedNode(DEC_LIFECYCLE_STATE),
                None, None,
            )
        )
        assert len(rows) == 1
        assert FEEDBACK_STATE_PRODUCED in str(rows[0].object)


# --------------------------------------------------------------------------- #
# Quad-builder defensive surface                                              #
# --------------------------------------------------------------------------- #


def test_feedback_quads_carry_required_predicates() -> None:
    emission = FeedbackEmission(
        feedback_class="gap",
        evidence=_evidence("required predicates check"),
        recommendation="Author the missing spec section.",
    )
    quads = build_feedback_quads(
        iri="urn:dec:feedback:fixed-iri",
        emission=emission,
        source_session_iri="urn:dec:session:fixed",
    )
    by_predicate: dict[str, list[pyoxigraph.Quad]] = {}
    for q in quads:
        by_predicate.setdefault(str(q.predicate), []).append(q)

    # Every Feedback artifact carries at least these predicates:
    for required in (
        DEC_FEEDBACK_CLASS,
        DEC_LIFECYCLE_STATE,
        DEC_TARGET_ROLE,
        DEC_EVIDENCE,
        DEC_SOURCE_SESSION,
    ):
        assert f"<{required}>" in by_predicate, (
            f"missing predicate {required} in {sorted(by_predicate)}"
        )
    # rdf:type → dec:Feedback wires the artifact to the class.
    type_quads = by_predicate[
        "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>"
    ]
    assert any(str(q.object) == f"<{DEC_FEEDBACK}>" for q in type_quads)
    # Optional fields ride along when set.
    assert f"<{DEC_RECOMMENDATION}>" in by_predicate


def test_judgment_quads_link_back_to_session() -> None:
    quads = build_judgment_quads(
        iri="urn:dec:emergent-judgment:fixed",
        decision="pick lib-A over lib-B",
        rationale="lib-A is on the dependency allow-list",
        source_session_iri="urn:dec:session:fixed",
    )
    triples = {(str(q.subject), str(q.predicate), str(q.object)) for q in quads}
    # Source-session predicate must point at the supplied session IRI.
    assert (
        "<urn:dec:emergent-judgment:fixed>",
        f"<{DEC_SOURCE_SESSION}>",
        "<urn:dec:session:fixed>",
    ) in triples


def test_emission_defaults_match_adrs() -> None:
    """The blocking + target-role defaults match ADR-023 + ADR-026 tables."""
    for cls in FEEDBACK_CLASSES:
        emission = FeedbackEmission(
            feedback_class=cls,  # type: ignore[arg-type]
            evidence=_evidence(f"checking defaults for {cls}"),
        )
        assert emission_is_blocking(emission) is CLASS_BLOCKING_DEFAULTS[cls]
        assert emission_target_role(emission) == CLASS_TARGET_ROLE_DEFAULTS[cls]


def test_emission_rejects_short_evidence() -> None:
    with pytest.raises(ValueError):
        FeedbackEmission(feedback_class="gap", evidence="too short")


def test_emission_rejects_unknown_severity() -> None:
    with pytest.raises(ValueError):
        FeedbackEmission(
            feedback_class="gap",
            evidence=_evidence("severity check"),
            severity="catastrophic",
        )


def test_emit_feedback_refused_after_completion() -> None:
    session = Session(_make_dispatch())
    session.build_completion()
    with pytest.raises(RuntimeError, match="already produced a completion"):
        session.emit_feedback(
            feedback_class="defect",
            evidence=_evidence("called after completion"),
        )


# --------------------------------------------------------------------------- #
# Outcome routing through CompletionPayload                                    #
# --------------------------------------------------------------------------- #


def test_completion_payload_carries_session_outcome_round_trip() -> None:
    session = Session(_make_dispatch(session_iri="urn:dec:session:roundtrip"))
    session.record_emergent_judgment(
        decision="add a TODO comment marker on the helper",
        rationale="naming-within-feature is in-authority for the implementer.",
    )
    session.emit_feedback(
        feedback_class="capability-request",
        evidence=_evidence("would benefit from a read_adjacent_file tool"),
    )
    payload = session.build_completion()
    assert isinstance(payload, CompletionPayload)
    assert payload.outcome == OUTCOME_SUCCESS
    # Telemetry still includes structural counts.
    assert payload.telemetry["side_channel_quad_count"] >= 7
    # The judgment lives on the artifact set, not the side-channel.
    assert payload.telemetry["artifact_quad_count"] >= 5
