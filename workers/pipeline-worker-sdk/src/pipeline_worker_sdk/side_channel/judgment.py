"""Emergent-judgment quad builder for in-authority worker decisions (FT-082)."""

from __future__ import annotations

import uuid
from datetime import datetime, timezone

import pyoxigraph

from .vocab import (
    DEC_DECISION,
    DEC_EMERGENT_JUDGMENT,
    DEC_RATIONALE,
    DEC_RECORDED_AT,
    DEC_SOURCE_SESSION,
    ORCHESTRATION_GRAPH,
    RDF_TYPE,
)


def mint_judgment_iri() -> str:
    """Generate a fresh URN IRI for a `dec:EmergentJudgment` artifact."""
    return f"urn:dec:emergent-judgment:{uuid.uuid4()}"


def _now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def _lit(
    subject: pyoxigraph.NamedNode,
    predicate_iri: str,
    value: str,
    graph: pyoxigraph.NamedNode,
) -> pyoxigraph.Quad:
    return pyoxigraph.Quad(
        subject,
        pyoxigraph.NamedNode(predicate_iri),
        pyoxigraph.Literal(value),
        graph,
    )


def _named(
    subject: pyoxigraph.NamedNode,
    predicate_iri: str,
    object_iri: str,
    graph: pyoxigraph.NamedNode,
) -> pyoxigraph.Quad:
    return pyoxigraph.Quad(
        subject,
        pyoxigraph.NamedNode(predicate_iri),
        pyoxigraph.NamedNode(object_iri),
        graph,
    )


def build_judgment_quads(
    *,
    iri: str,
    decision: str,
    rationale: str,
    source_session_iri: str,
    graph: str = ORCHESTRATION_GRAPH,
    recorded_at: str | None = None,
) -> list[pyoxigraph.Quad]:
    """Materialise a `dec:EmergentJudgment` as pyoxigraph quads.

    Per FT-082 the judgment is metadata on the produced artifact; the
    session attaches the quads to its ``_artifact`` emission set so the
    paired interpretation session sees them on the same bundle as the
    main artifact.
    """
    if not decision.strip():
        raise ValueError("decision must not be blank")
    if not rationale.strip():
        raise ValueError("rationale must not be blank")
    subj = pyoxigraph.NamedNode(iri)
    g = pyoxigraph.NamedNode(graph)
    ts = recorded_at or _now_iso()
    return [
        _named(subj, RDF_TYPE, DEC_EMERGENT_JUDGMENT, g),
        _lit(subj, DEC_DECISION, decision, g),
        _lit(subj, DEC_RATIONALE, rationale, g),
        _named(subj, DEC_SOURCE_SESSION, source_session_iri, g),
        _lit(subj, DEC_RECORDED_AT, ts, g),
    ]


__all__ = [
    "build_judgment_quads",
    "mint_judgment_iri",
]
