"""Shared worker bundle model exposing the FT-030 role authority field."""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, Field


class EscalationHint(BaseModel):
    """Per-mustEscalate-category routing hint (ADR-027)."""

    category: str = Field(
        ...,
        description="Authority category name (matches a mustEscalate value).",
    )
    feedback_class: Literal[
        "gap",
        "contradiction",
        "unimplementable",
        "scope-issue",
        "defect",
        "capability-request",
    ] = Field(
        ...,
        alias="class",
        description="Feedback class (ADR-023) the escalation should be emitted as.",
        validation_alias="class",
        serialization_alias="class",
    )
    target_role: str = Field(
        ...,
        description="Default target role for the escalation (ADR-027).",
    )

    model_config = {"populate_by_name": True}


class Authority(BaseModel):
    """A role's bounded authority declaration (ADR-027 / FT-030).

    Workers consume this from the dispatch payload via ``Bundle.authority``.
    The ``mayDecide`` / ``mustEscalate`` lists tell the worker which
    judgments are in-scope and which must be escalated as feedback.
    """

    iri: str = Field(
        default="",
        description="IRI of the dec:Authority artifact (audit / introspection).",
    )
    may_decide: list[str] = Field(
        default_factory=list,
        description="Category names the worker MAY decide unilaterally.",
    )
    must_escalate: list[str] = Field(
        default_factory=list,
        description="Category names the worker MUST escalate via feedback.",
    )
    escalate_via: list[EscalationHint] = Field(
        default_factory=list,
        description="Per-category routing hints.",
    )
    rationale: str = Field(
        default="",
        description="Human-readable explanation of the boundary.",
    )


class Bundle(BaseModel):
    """The minimal worker-facing bundle shape carrying FT-030 authority data.

    Slice-specific workers (``code-writer``, ``verifier``) extend this
    base with their own context fields (``bundle_markdown``,
    ``produced_artifact``, …). This shared shell exposes only what every
    worker needs to consult the authority declaration: the role id and
    the authority itself.
    """

    role_id: str = Field(
        ...,
        description="Stable role identifier (e.g. 'code-writer').",
    )
    authority: Authority | None = Field(
        default=None,
        description="Role authority declaration (ADR-027). None on legacy stores.",
    )
