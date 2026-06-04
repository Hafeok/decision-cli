"""Serialises a validated AdrProposal to the harness JSON shape."""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

from .bundle import PreflightGapRecord


ProposalKind = Literal["new", "acknowledgement", "gap"]

AdrScope = Literal["cross-cutting", "platform", "domain", "feature-specific"]


class NewAdrProposal(BaseModel):
    """The worker proposes a complete, H2-conforming net-new ADR body."""

    model_config = ConfigDict(extra="forbid")

    title: str = Field(
        ...,
        min_length=1,
        description="Title for the proposed ADR.",
    )
    body: str = Field(
        ...,
        min_length=1,
        description=(
            "Full markdown body. MUST contain every H2 section from "
            "adr_body_schema.required_h2_sections."
        ),
    )
    scope: AdrScope = Field(
        ...,
        description="ADR scope ('cross-cutting', 'platform', 'domain', 'feature-specific').",
    )
    proposed_domains: list[str] = Field(
        default_factory=list,
        description="Domain names from domain_registry the ADR belongs to.",
    )
    addresses_gap: PreflightGapRecord = Field(
        ...,
        description="Echo of the preflight_gap this ADR closes.",
    )
    rationale: str = Field(
        ...,
        min_length=20,
        description="Why this ADR addresses the gap and respects the central ADRs.",
    )


class AcknowledgementProposal(BaseModel):
    """The worker proposes an acknowledgement linking an existing ADR or domain.

    Brief §4B is explicit: every acknowledgement MUST carry substantive
    `reasoning` (≥ 40 characters per FT-130 §Behaviour). Bare
    acknowledgements are rejected at the worker boundary BEFORE stdout.
    """

    model_config = ConfigDict(extra="forbid")

    acknowledges: str = Field(
        ...,
        min_length=1,
        description="ADR identifier (e.g. 'ADR-013') or domain name being acknowledged.",
    )
    target_feature: str = Field(
        ...,
        min_length=1,
        description="Feature this acknowledgement applies to (e.g. 'FT-101').",
    )
    reasoning: str = Field(
        ...,
        min_length=40,
        description=(
            "Substantive reasoning explaining why the acknowledged ADR/domain "
            "governs the feature. MUST be ≥ 40 chars per FT-130 §4B."
        ),
    )
    rationale: str = Field(
        ...,
        min_length=20,
        description="Why this acknowledgement closes the preflight gap.",
    )


class GapProposal(BaseModel):
    """Neither a net-new ADR nor a reasoned acknowledgement is defensible.

    The worker emits gap when the bundle under-specifies the decision
    space — the planner routes this upstream for human enrichment.
    """

    model_config = ConfigDict(extra="forbid")

    missing_information: list[str] = Field(
        ...,
        min_length=1,
        description="Concrete items the brief should clarify before authoring is possible.",
    )
    reason: str = Field(
        ...,
        min_length=1,
        description="Why the worker cannot produce a defensible ADR or acknowledgement.",
    )


class AdrProposal(BaseModel):
    """Single artifact returned by the adr-author worker (ADR-073).

    Exactly one of `new` / `acknowledgement` / `gap` is populated; `kind`
    is the discriminator. `bundle_hash` echoes the input hash so the
    harness can verify provenance (per ADR-073).
    """

    model_config = ConfigDict(extra="forbid")

    kind: ProposalKind = Field(..., description="Proposal kind discriminator.")
    bundle_hash: str = Field(
        ...,
        min_length=8,
        description="Echo of input bundle_hash for verification.",
    )
    new: NewAdrProposal | None = Field(
        default=None,
        description="Populated when kind='new'.",
    )
    acknowledgement: AcknowledgementProposal | None = Field(
        default=None,
        description="Populated when kind='acknowledgement'.",
    )
    gap: GapProposal | None = Field(
        default=None,
        description="Populated when kind='gap'.",
    )
