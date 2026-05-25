"""Feedback emission model + quad builder for the worker side-channel (FT-082)."""

from __future__ import annotations

import uuid
from typing import Literal

import pyoxigraph
from pydantic import BaseModel, Field, field_validator

from .vocab import (
    CLASS_BLOCKING_DEFAULTS,
    CLASS_TARGET_ROLE_DEFAULTS,
    DEC_DISPOSITION_OVERRIDE,
    DEC_DISPOSITION_RATIONALE,
    DEC_EVIDENCE,
    DEC_FEEDBACK,
    DEC_FEEDBACK_CLASS,
    DEC_LIFECYCLE_STATE,
    DEC_RECOMMENDATION,
    DEC_SEVERITY,
    DEC_SOURCE_SESSION,
    DEC_TARGET_ROLE,
    FEEDBACK_CLASSES,
    FEEDBACK_STATE_PRODUCED,
    ORCHESTRATION_GRAPH,
    RDF_TYPE,
    SEVERITIES,
)

#: Literal type alias enumerating the six ADR-023 feedback classes.
FeedbackClass = Literal[
    "gap",
    "contradiction",
    "unimplementable",
    "scope-issue",
    "defect",
    "capability-request",
]


class FeedbackEmission(BaseModel):
    """A single worker feedback emission (ADR-022 / ADR-023 / ADR-025).

    The shape mirrors `core::worker::ipc::feedback::FeedbackEmission` so the
    harness can lift it into a `dec:Feedback` artifact via `StreamWriter`
    after SHACL validation. Workers should treat this as a transient
    record — once emitted into a `Session`, the canonical record is the
    quads in the session's side-channel store, not the model instance.
    """

    feedback_class: FeedbackClass = Field(
        ...,
        description="ADR-023 controlled-vocabulary class tag.",
    )
    severity: str = Field(
        default="warning",
        description="Severity hint (info|warning|error or low|medium|high).",
    )
    evidence: str = Field(
        ...,
        min_length=20,
        description="Free-form citation into the bundle (≥ 20 chars).",
    )
    recommendation: str | None = Field(
        default=None,
        description="Optional suggested fix for the target role.",
    )
    target_role: str | None = Field(
        default=None,
        description="Per-emission target-role override (defaults per ADR-026).",
    )
    blocking: bool | None = Field(
        default=None,
        description="Per-emission blocking override (None = class default).",
    )
    disposition_rationale: str | None = Field(
        default=None,
        description="Rationale when blocking diverges from the class default.",
    )

    @field_validator("evidence")
    @classmethod
    def _evidence_not_blank(cls, value: str) -> str:
        if not value.strip():
            raise ValueError("evidence must not be blank")
        return value

    @field_validator("severity")
    @classmethod
    def _severity_known(cls, value: str) -> str:
        if value not in SEVERITIES:
            raise ValueError(
                f"severity must be one of {SEVERITIES!r}; got {value!r}"
            )
        return value


def mint_feedback_iri() -> str:
    """Generate a fresh URN IRI for a new `dec:Feedback` artifact."""
    return f"urn:dec:feedback:{uuid.uuid4()}"


def emission_is_blocking(emission: FeedbackEmission) -> bool:
    """Resolve the effective blocking flag for an emission (ADR-025).

    Per-emission `blocking` overrides the class default; `None` falls
    back to the class default from ADR-023's vocabulary table.
    """
    if emission.blocking is not None:
        return emission.blocking
    return CLASS_BLOCKING_DEFAULTS[emission.feedback_class]


def emission_target_role(emission: FeedbackEmission) -> str:
    """Resolve the effective target role for an emission (ADR-026)."""
    if emission.target_role:
        return emission.target_role
    return CLASS_TARGET_ROLE_DEFAULTS[emission.feedback_class]


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


def _required_quads(
    subj: pyoxigraph.NamedNode,
    emission: FeedbackEmission,
    source_session_iri: str,
    graph: pyoxigraph.NamedNode,
) -> list[pyoxigraph.Quad]:
    target_role = emission_target_role(emission)
    return [
        _named(subj, RDF_TYPE, DEC_FEEDBACK, graph),
        _lit(subj, DEC_FEEDBACK_CLASS, emission.feedback_class, graph),
        _lit(subj, DEC_LIFECYCLE_STATE, FEEDBACK_STATE_PRODUCED, graph),
        _lit(subj, DEC_TARGET_ROLE, target_role, graph),
        _lit(subj, DEC_EVIDENCE, emission.evidence, graph),
        _lit(subj, DEC_SEVERITY, emission.severity, graph),
        _named(subj, DEC_SOURCE_SESSION, source_session_iri, graph),
    ]


def _optional_quads(
    subj: pyoxigraph.NamedNode,
    emission: FeedbackEmission,
    graph: pyoxigraph.NamedNode,
) -> list[pyoxigraph.Quad]:
    out: list[pyoxigraph.Quad] = []
    if emission.recommendation:
        out.append(_lit(subj, DEC_RECOMMENDATION, emission.recommendation, graph))
    class_default = CLASS_BLOCKING_DEFAULTS[emission.feedback_class]
    effective = emission_is_blocking(emission)
    if emission.blocking is not None and effective != class_default:
        override = "blocking" if effective else "non-blocking"
        out.append(_lit(subj, DEC_DISPOSITION_OVERRIDE, override, graph))
        if emission.disposition_rationale:
            out.append(
                _lit(subj, DEC_DISPOSITION_RATIONALE,
                     emission.disposition_rationale, graph)
            )
    return out


def build_feedback_quads(
    *,
    iri: str,
    emission: FeedbackEmission,
    source_session_iri: str,
    graph: str = ORCHESTRATION_GRAPH,
) -> list[pyoxigraph.Quad]:
    """Materialise a `dec:Feedback` artifact as pyoxigraph quads.

    Mirrors `core::feedback::artifact::Feedback::to_quads` on the harness
    side; SHACL validation runs at the GraphWriter chokepoint (ADR-041),
    so the worker emits the artifact in lifecycle state ``produced`` and
    lets the harness mature it.
    """
    subj = pyoxigraph.NamedNode(iri)
    g = pyoxigraph.NamedNode(graph)
    quads = _required_quads(subj, emission, source_session_iri, g)
    quads.extend(_optional_quads(subj, emission, g))
    return quads


__all__ = [
    "FEEDBACK_CLASSES",
    "FeedbackClass",
    "FeedbackEmission",
    "build_feedback_quads",
    "emission_is_blocking",
    "emission_target_role",
    "mint_feedback_iri",
]
