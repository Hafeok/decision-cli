"""TC-145: Typed artifact builders with SHACL validation at commit (FT-080).

Covers the three success criteria the parent feature_spec names:

1. A builder missing a required field — including a required motivational
   predicate — raises on ``commit()`` with a SHACL-derived error message,
   before any wire send.
2. A builder that passes local SHACL-derived validation produces triples
   shaped exactly as the per-type shape (FT-072) declares — the harness's
   GraphWriter re-validates authoritatively (FT-073), so the union of
   local + harness checks closes the boundary with no semantic drift.
3. Calls to ``emit_triple`` increment a telemetry counter visible on the
   completion event (``artifact_escape_hatch_count``).

Defensive validation lives in :class:`BuilderBase`; the per-shape
specifics come from the generated typed builders under
``pipeline_worker_sdk.artifact._generated``.
"""

from __future__ import annotations

import pyoxigraph
import pytest

from pipeline_worker_sdk import (
    BuilderBase,
    CommitError,
    DispatchEvent,
    OUTCOME_BLOCKED,
    OUTCOME_SUCCESS,
    Session,
)
from pipeline_worker_sdk.artifact._base import (
    DEC_BOUNDARY_ARTIFACT,
    DEC_EXTERNAL_ORIGIN,
)
from pipeline_worker_sdk.artifact._generated.adr import ADRBuilder
from pipeline_worker_sdk.artifact._generated.discovery_finding import (
    DiscoveryFindingBuilder,
)
from pipeline_worker_sdk.artifact._generated.dispatch import DispatchBuilder
from pipeline_worker_sdk.artifact._generated.feature import FeatureBuilder
from pipeline_worker_sdk.artifact._generated.worker_image_submission import (
    WorkerImageSubmissionBuilder,
)


# --------------------------------------------------------------------------- #
# Helpers                                                                      #
# --------------------------------------------------------------------------- #


def _make_dispatch(
    *,
    dispatch_id: str = "urn:dec:dispatch:tc145",
    session_iri: str | None = None,
    nquads: str = "",
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
# Criterion 1 — missing required field / missing motivational raises.          #
# --------------------------------------------------------------------------- #


def test_missing_motivational_raises_with_shacl_derived_message() -> None:
    builder = FeatureBuilder("urn:dec:feature:FT-MISSING")
    with pytest.raises(CommitError) as info:
        builder.commit()
    msg = str(info.value)
    # SHACL-derived: names the shape, the focal IRI, and every
    # motivational alternative the shape declared.
    assert "dec:Feature" in msg
    assert "urn:dec:feature:FT-MISSING" in msg
    assert "addresses → dec:Feedback" in msg
    assert "decomposesFrom → dec:Brief" in msg
    assert "originatedFrom → dec:DiscoveryFinding" in msg
    assert "respondsTo → dec:Question" in msg
    assert "BoundaryArtifact" in msg
    assert info.value.target_class_local == "Feature"
    assert info.value.focus_iri == "urn:dec:feature:FT-MISSING"


def test_worker_image_submission_requires_boundary_when_no_motivational() -> None:
    """Shapes with no motivational alternatives but accepts_boundary=True
    must be marked as BoundaryArtifact on commit."""
    builder = WorkerImageSubmissionBuilder("urn:dec:wis:1")
    with pytest.raises(CommitError) as info:
        builder.commit()
    msg = str(info.value)
    assert "dec:WorkerImageSubmission" in msg
    assert "BoundaryArtifact" in msg


def test_motivational_satisfied_by_any_alternative() -> None:
    """Adding any one motivational predicate satisfies the sh:or."""
    for setter, target in [
        ("add_addresses", "urn:dec:feedback:fb1"),
        ("add_decomposesFrom", "urn:dec:brief:br1"),
        ("add_originatedFrom", "urn:dec:discovery:df1"),
        ("add_respondsTo", "urn:dec:question:q1"),
    ]:
        b = FeatureBuilder(f"urn:dec:feature:FT-{setter}")
        getattr(b, setter)(target)
        triples = b.commit()
        # Must contain rdf:type and the motivational edge.
        assert any(
            t[1] == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
            for t in triples
        )
        assert any(t[2] == target for t in triples)


def test_boundary_artifact_satisfies_motivational_requirement() -> None:
    builder = FeatureBuilder("urn:dec:feature:FT-BOUNDARY")
    builder.mark_boundary_artifact(external_origin="chat-transcript-2026-05-25")
    triples = builder.commit()
    # BoundaryArtifact class triple AND external_origin triple are present.
    type_targets = {t[2] for t in triples if t[1].endswith("#type")}
    assert "https://decision-cli.dev/ns#Feature" in type_targets
    assert DEC_BOUNDARY_ARTIFACT in type_targets
    origins = [t for t in triples if t[1] == DEC_EXTERNAL_ORIGIN]
    assert origins == [
        (
            "urn:dec:feature:FT-BOUNDARY",
            DEC_EXTERNAL_ORIGIN,
            "chat-transcript-2026-05-25",
        )
    ]


def test_boundary_artifact_rejected_on_shape_that_does_not_accept_it() -> None:
    """Dispatch's shape has accepts_boundary=False; mark_boundary raises."""
    builder = DispatchBuilder("urn:dec:dispatch:nope")
    with pytest.raises(CommitError, match="does not accept the BoundaryArtifact"):
        builder.mark_boundary_artifact(external_origin="ci-run-xyz")


def test_boundary_artifact_requires_external_origin() -> None:
    builder = FeatureBuilder("urn:dec:feature:FT-NoOrigin")
    with pytest.raises(ValueError, match="external_origin"):
        builder.mark_boundary_artifact(external_origin="")
    with pytest.raises(ValueError, match="external_origin"):
        builder.mark_boundary_artifact(external_origin="   ")


def test_dispatch_commit_skips_motivational_check() -> None:
    """The Dispatch shape has no sh:or — commit must succeed with bare type."""
    builder = DispatchBuilder("urn:dec:dispatch:tc145-ok")
    triples = builder.commit()
    assert triples == [
        (
            "urn:dec:dispatch:tc145-ok",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            "https://decision-cli.dev/ns#Dispatch",
        )
    ]


# --------------------------------------------------------------------------- #
# Criterion 2 — local validation matches the per-type shape.                   #
# --------------------------------------------------------------------------- #


def test_adr_builder_commit_emits_motivational_edge_with_correct_iri() -> None:
    """ADR's motivational alternatives include addresses/decidesFor/supersedes."""
    builder = ADRBuilder("urn:dec:adr:ADR-1")
    builder.add_decidesFor("urn:dec:feature:FT-1")
    triples = builder.commit()
    by_pred = {(s, p): o for s, p, o in triples}
    assert by_pred[
        ("urn:dec:adr:ADR-1", "https://decision-cli.dev/ns#decidesFor")
    ] == "urn:dec:feature:FT-1"


def test_discovery_finding_satisfies_motivational_via_derived_from() -> None:
    builder = DiscoveryFindingBuilder("urn:dec:discovery:1")
    builder.add_derivedFrom("urn:dec:sensing-action:1")
    triples = builder.commit()
    assert any(
        t[1] == "https://decision-cli.dev/ns#derivedFrom" for t in triples
    )


def test_committed_triples_are_shacl_subset_of_shape() -> None:
    """The per-type shape (FT-072) governs which predicates the builder may
    emit; this test asserts we don't introduce out-of-shape predicates by
    accident. The harness re-validates authoritatively per FT-073."""
    builder = FeatureBuilder("urn:dec:feature:FT-SHAPED")
    builder.add_decomposesFrom("urn:dec:brief:b1")
    builder.add_addresses("urn:dec:feedback:fb1")
    triples = builder.commit()
    allowed_predicates = {
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
        FeatureBuilder.P_addresses,
        FeatureBuilder.P_decomposesFrom,
        FeatureBuilder.P_originatedFrom,
        FeatureBuilder.P_respondsTo,
    }
    for _s, p, _o in triples:
        assert p in allowed_predicates, f"unexpected predicate {p!r}"


def test_double_commit_is_a_programming_error() -> None:
    builder = FeatureBuilder("urn:dec:feature:FT-DOUBLE")
    builder.add_decomposesFrom("urn:dec:brief:b1")
    builder.commit()
    with pytest.raises(RuntimeError, match="already committed"):
        builder.commit()


# --------------------------------------------------------------------------- #
# Criterion 3 — emit_triple bumps a telemetry counter on completion.           #
# --------------------------------------------------------------------------- #


def test_emit_triple_increments_builder_escape_hatch_count() -> None:
    builder = FeatureBuilder("urn:dec:feature:FT-ESC1")
    assert builder.escape_hatch_count == 0
    builder.add_decomposesFrom("urn:dec:brief:b1")
    builder.emit_triple(
        "urn:dec:feature:FT-ESC1",
        "http://example.org/note",
        "extra payload",
    )
    builder.emit_triple(
        "urn:dec:feature:FT-ESC1",
        "http://example.org/note2",
        "more",
    )
    assert builder.escape_hatch_count == 2
    triples = builder.commit()
    objs = [o for _, _, o in triples]
    assert "extra payload" in objs
    assert "more" in objs


def test_emit_triple_counter_surfaces_on_completion_event() -> None:
    """The Session.commit_artifact() path aggregates builder escape-hatch
    usage onto the completion event's telemetry block per FT-080 §criterion 3."""
    session = Session(_make_dispatch(session_iri="urn:dec:session:tc145-esc"))

    b1 = FeatureBuilder("urn:dec:feature:FT-A")
    b1.add_decomposesFrom("urn:dec:brief:b1")
    b1.emit_triple("urn:dec:feature:FT-A", "http://example.org/note", "x")
    session.commit_artifact(b1)

    b2 = ADRBuilder("urn:dec:adr:ADR-X")
    b2.add_decidesFor("urn:dec:feature:FT-A")
    b2.emit_triple("urn:dec:adr:ADR-X", "http://example.org/note", "y")
    b2.emit_triple("urn:dec:adr:ADR-X", "http://example.org/note2", "z")
    session.commit_artifact(b2)

    assert session.artifact_escape_hatch_count == 3
    assert session.artifact_commits == 2

    payload = session.build_completion()
    assert payload.outcome == OUTCOME_SUCCESS
    assert payload.telemetry["artifact_escape_hatch_count"] == 3
    assert payload.telemetry["artifact_commits"] == 2


def test_link_to_does_not_bump_escape_hatch_counter() -> None:
    """``link_to`` is a typed convenience for forward edges; it does NOT
    increment the gap-surface escape-hatch counter."""
    builder = FeatureBuilder("urn:dec:feature:FT-LINK")
    builder.add_decomposesFrom("urn:dec:brief:b1")
    builder.link_to(
        "urn:dec:adr:ADR-1",
        predicate="https://decision-cli.dev/ns#governs",
    )
    builder.commit()
    assert builder.escape_hatch_count == 0


def test_emit_triple_rejects_empty_subject_or_predicate() -> None:
    builder = FeatureBuilder("urn:dec:feature:FT-X")
    with pytest.raises(ValueError):
        builder.emit_triple("", "http://example.org/p", "v")
    with pytest.raises(ValueError):
        builder.emit_triple("urn:dec:feature:FT-X", "", "v")


def test_link_to_validates_iri_shape() -> None:
    builder = FeatureBuilder("urn:dec:feature:FT-X2")
    with pytest.raises(ValueError):
        builder.link_to("", predicate="http://example.org/p")
    with pytest.raises(ValueError):
        builder.link_to("urn:dec:adr:ADR-1", predicate="")
    with pytest.raises(ValueError, match="qualified IRI"):
        builder.link_to("not-an-iri", predicate="http://example.org/p")


# --------------------------------------------------------------------------- #
# Session integration — commit_artifact wires builder → session sub-store.     #
# --------------------------------------------------------------------------- #


def test_commit_artifact_writes_triples_into_session_artifact_store() -> None:
    session = Session(
        _make_dispatch(session_iri="urn:dec:session:tc145-write"),
    )
    builder = FeatureBuilder("urn:dec:feature:FT-WRITE")
    builder.add_decomposesFrom("urn:dec:brief:b1")
    count = session.commit_artifact(builder)
    assert count == 2  # rdf:type + decomposesFrom
    assert session.artifact_size == 2

    payload = session.build_completion()
    quads = _parse_nquads(payload.nquads_payload)
    assert len(quads) == 2
    subjects = {str(q.subject) for q in quads}
    assert "<urn:dec:feature:FT-WRITE>" in subjects


def test_commit_artifact_propagates_commit_error_unconverted() -> None:
    """If the builder fails SHACL-derived validation, commit_artifact
    surfaces the same CommitError so the worker can transition to
    blocked-completion telemetry."""
    session = Session(_make_dispatch())
    builder = FeatureBuilder("urn:dec:feature:FT-NEVER")
    with pytest.raises(CommitError):
        session.commit_artifact(builder)
    # Session is not closed — the worker can still build a blocked completion.
    assert not session.closed
    payload = session.build_blocked_completion(error="missing motivational")
    assert payload.outcome == OUTCOME_BLOCKED


def test_commit_artifact_rejects_non_builder() -> None:
    session = Session(_make_dispatch())
    with pytest.raises(TypeError):
        session.commit_artifact("not a builder")  # type: ignore[arg-type]


def test_commit_artifact_uses_explicit_graph_iri_when_provided() -> None:
    session = Session(_make_dispatch())
    builder = FeatureBuilder("urn:dec:feature:FT-CUSTOM")
    builder.add_decomposesFrom("urn:dec:brief:b1")
    session.commit_artifact(builder, graph_iri="urn:dec:custom-graph")
    payload = session.build_completion()
    quads = _parse_nquads(payload.nquads_payload)
    assert all(str(q.graph_name) == "<urn:dec:custom-graph>" for q in quads)


# --------------------------------------------------------------------------- #
# Defensive — no mechanical provenance emitted by the worker (ADR-050).         #
# --------------------------------------------------------------------------- #


def test_committed_triples_carry_no_mechanical_provenance() -> None:
    """ADR-050: workers must NOT emit prov:wasGeneratedBy /
    prov:wasAttributedTo / prov:generatedAtTime — the harness materialises
    those from the session record at write time. The builder must not
    sneak them in."""
    builder = FeatureBuilder("urn:dec:feature:FT-NoMech")
    builder.add_decomposesFrom("urn:dec:brief:b1")
    triples = builder.commit()
    forbidden = {
        "http://www.w3.org/ns/prov#wasGeneratedBy",
        "http://www.w3.org/ns/prov#wasAttributedTo",
        "http://www.w3.org/ns/prov#generatedAtTime",
    }
    used_predicates = {p for _s, p, _o in triples}
    assert not (used_predicates & forbidden), (
        "worker-side builder must not emit mechanical-provenance triples "
        "per ADR-050; harness does that at write time."
    )


# --------------------------------------------------------------------------- #
# Builder base introspection — class-level shape metadata is queryable.        #
# --------------------------------------------------------------------------- #


def test_builder_class_exposes_shape_metadata() -> None:
    """Workers (and the harness) can introspect a builder class for the
    source shape file, accepts_boundary flag, and motivational descriptors
    without instantiating one."""
    assert FeatureBuilder.TARGET_CLASS_IRI == (
        "https://decision-cli.dev/ns#Feature"
    )
    assert FeatureBuilder.TARGET_CLASS_LOCAL == "Feature"
    assert FeatureBuilder.SOURCE_SHAPE == "workers/_shared/shapes/feature.ttl"
    assert FeatureBuilder.ACCEPTS_BOUNDARY is True
    predicates = {m.predicate_local for m in FeatureBuilder.MOTIVATIONAL}
    assert predicates == {"addresses", "decomposesFrom", "originatedFrom", "respondsTo"}


def test_all_generated_builders_inherit_builderbase() -> None:
    """Sanity: every generated builder is a BuilderBase subclass so commit()
    semantics are uniform across types."""
    from pipeline_worker_sdk.artifact import _generated as gen
    for name in gen.__all__:
        cls = getattr(gen, name)
        assert issubclass(cls, BuilderBase), f"{name} not a BuilderBase"
        # And every concrete builder exposes the same commit() entry point.
        assert callable(getattr(cls, "commit", None))
