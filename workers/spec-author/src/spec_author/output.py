"""Serialises a validated SpecProposal to the harness JSON shape."""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field


ProposalKind = Literal["new", "gap"]


class NewSpecProposal(BaseModel):
    """The worker proposes a complete, schema-conforming feature_spec body."""

    model_config = ConfigDict(extra="forbid")

    title: str = Field(
        ...,
        min_length=1,
        description="Title for the proposed feature_spec.",
    )
    body: str = Field(
        ...,
        min_length=1,
        description=(
            "Full markdown body. MUST contain every H2 section from "
            "body_schema.required_h2_sections and every H3 subsection from "
            "body_schema.required_h3_subsections under Functional Specification."
        ),
    )
    proposed_depends_on: list[str] = Field(
        default_factory=list,
        description="FT-NNN identifiers this spec should depend on.",
    )
    proposed_adrs: list[str] = Field(
        default_factory=list,
        description="ADR-NNN identifiers the spec links to / respects.",
    )
    proposed_domains: list[str] = Field(
        default_factory=list,
        description="Domain names from domain_registry the spec belongs to.",
    )
    rationale: str = Field(
        ...,
        min_length=20,
        description="Why this body addresses the request and respects the central ADRs.",
    )


class GapProposal(BaseModel):
    """The brief is too under-specified to author a defensible spec."""

    model_config = ConfigDict(extra="forbid")

    missing_information: list[str] = Field(
        ...,
        min_length=1,
        description="Concrete items the brief should clarify before authoring is possible.",
    )
    reason: str = Field(
        ...,
        min_length=1,
        description="Why the worker cannot produce a defensible spec from the current bundle.",
    )


class SpecProposal(BaseModel):
    """Single artifact returned by the spec-author worker (ADR-073).

    Exactly one of `new` / `gap` is populated; `kind` is the discriminator.
    `bundle_hash` echoes the input hash so the harness can verify provenance
    (per ADR-073).
    """

    model_config = ConfigDict(extra="forbid")

    kind: ProposalKind = Field(..., description="Proposal kind discriminator.")
    bundle_hash: str = Field(
        ...,
        min_length=8,
        description="Echo of input bundle_hash for verification.",
    )
    new: NewSpecProposal | None = Field(
        default=None,
        description="Populated when kind='new'.",
    )
    gap: GapProposal | None = Field(
        default=None,
        description="Populated when kind='gap'.",
    )
