"""Parses an incoming spec-quality dispatch bundle into a SpecQualityInput struct."""

from __future__ import annotations

from typing import Optional

from pydantic import BaseModel, Field


class RequestRecord(BaseModel):
    """The originating brief / request the spec must address."""

    id: str = Field(..., description="Request identifier, e.g. 'REQ-007'.")
    title: str = Field(default="", description="Short request title.")
    body: str = Field(default="", description="Full markdown body of the request brief.")
    source: str = Field(
        default="",
        description="Origin marker (e.g. 'product request apply', 'human conversation').",
    )


class BodySchemaRecord(BaseModel):
    """The H2/H3 contract for feature_spec bodies from ADR-047."""

    required_h2_sections: list[str] = Field(
        default_factory=list,
        description="Required top-level H2 sections (e.g. ['Description', 'Functional Specification', 'Out of scope']).",
    )
    required_h3_subsections: list[str] = Field(
        default_factory=list,
        description=(
            "Required H3 subsections under Functional Specification "
            "(e.g. ['Inputs', 'Outputs', 'State', 'Behaviour', 'Invariants', 'Error handling', 'Boundaries'])."
        ),
    )
    section_descriptions: dict[str, str] = Field(
        default_factory=dict,
        description="Optional per-section guidance (section name -> short description).",
    )


class NewSpecRecord(BaseModel):
    """The new-kind payload mirroring spec-author's NewSpecProposal."""

    title: str = Field(default="", description="Title of the proposed feature_spec.")
    body: str = Field(default="", description="Full markdown body of the spec proposal.")
    proposed_depends_on: list[str] = Field(
        default_factory=list,
        description="FT-NNN identifiers the spec depends on.",
    )
    proposed_adrs: list[str] = Field(
        default_factory=list,
        description="ADR-NNN identifiers the spec respects / links to.",
    )
    proposed_domains: list[str] = Field(
        default_factory=list,
        description="Domain names from domain_registry the spec belongs to.",
    )
    rationale: str = Field(
        default="",
        description="Author's rationale for why the body addresses the request.",
    )


class GapSpecRecord(BaseModel):
    """The gap-kind payload mirroring spec-author's GapProposal."""

    missing_information: list[str] = Field(
        default_factory=list,
        description="Concrete items the brief should clarify before authoring is possible.",
    )
    reason: str = Field(
        default="",
        description="Why the worker cannot produce a defensible spec.",
    )


class SpecProposalRecord(BaseModel):
    """The SpecProposal artifact being judged (output of spec-author / FT-129)."""

    iri: str = Field(
        default="",
        description="IRI / identifier of the SpecProposal under judgment.",
    )
    kind: str = Field(
        ...,
        description="Proposal kind: 'new' or 'gap'.",
    )
    bundle_hash: str = Field(default="", description="Echoed bundle hash from spec-author.")
    new: Optional[NewSpecRecord] = Field(
        default=None,
        description="Present when kind='new'.",
    )
    gap: Optional[GapSpecRecord] = Field(
        default=None,
        description="Present when kind='gap'.",
    )


class FeatureRecord(BaseModel):
    """One related feature_spec, summarised for the judge's context."""

    id: str = Field(..., description="Feature identifier (e.g. 'FT-013').")
    title: str = Field(default="", description="Feature title.")
    description: str = Field(
        default="",
        description="Excerpt of the feature_spec's Description H2 section.",
    )
    boundaries: str = Field(
        default="",
        description="Excerpt of the feature_spec's Boundaries H3 subsection.",
    )


class AdrSummaryRecord(BaseModel):
    """One graph-central or cross-cutting ADR digest."""

    id: str = Field(..., description="ADR identifier (e.g. 'ADR-013').")
    title: str = Field(default="", description="ADR title.")
    scope: str = Field(default="", description="ADR scope (cross-cutting, slice, feature).")
    summary: str = Field(
        default="",
        description="One-paragraph digest of the ADR's Decision section.",
    )


class DomainRecord(BaseModel):
    """One domain in the controlled vocabulary from product.toml."""

    name: str = Field(..., description="Domain name (e.g. 'observability').")
    description: str = Field(
        default="",
        description="What this domain covers.",
    )


class SpecQualityRubricRecord(BaseModel):
    """The five criteria spec-quality scores against (FT-132)."""

    criteria: list[str] = Field(
        ...,
        description=(
            "List of criterion names: schema-conforming, request-faithful, "
            "bounded, non-colliding, linkage-sound."
        ),
    )
    description: str = Field(
        default="",
        description="Narrative description of the rubric.",
    )


class AuthorityRecord(BaseModel):
    """Authority declaration per ADR-027."""

    may_decide: list[str] = Field(
        default_factory=list,
        description="Categories the role may decide unilaterally.",
    )
    must_escalate: list[str] = Field(
        default_factory=list,
        description="Categories the role must escalate via feedback.",
    )
    escalate_via: list[dict] = Field(
        default_factory=list,
        description="Routing hints per escalation category.",
    )
    rationale: str = Field(
        default="",
        description="One-paragraph explanation of the authority scope.",
    )


class SpecQualityInput(BaseModel):
    """Inputs the harness hands the spec-quality judge for a single verdict.

    Matches the bundle FT-132's spec defines. The worker treats the
    bundle as a complete, self-contained context: it does NOT fetch
    additional files, query the graph, or make follow-up calls
    (ADR-008 / ADR-073).
    """

    spec_proposal: SpecProposalRecord = Field(
        ...,
        description="The SpecProposal artifact under judgment (output of FT-129).",
    )
    request: RequestRecord = Field(
        ...,
        description="The originating request / brief.",
    )
    body_schema: BodySchemaRecord = Field(
        ...,
        description="H2/H3 contract from ADR-047 and product.toml [features].",
    )
    related_features: list[FeatureRecord] = Field(
        default_factory=list,
        description="Nearby specs by domain or depends-on relationship (for non-redundancy check).",
    )
    central_adrs: list[AdrSummaryRecord] = Field(
        default_factory=list,
        description="Graph-central + cross-cutting ADRs the spec must respect.",
    )
    domain_registry: list[DomainRecord] = Field(
        default_factory=list,
        description="Allowed domain values from product.toml.",
    )
    rubric: SpecQualityRubricRecord = Field(
        ...,
        description="The five criteria spec-quality scores against.",
    )
    authority: AuthorityRecord = Field(
        ...,
        description="ADR-027 authority declaration for the spec-quality role.",
    )
    bundle_hash: str = Field(
        ...,
        min_length=8,
        description="SHA-256 hex of the canonical bundle (echoed in the verdict).",
    )
    endpoint: str = Field(
        default="anthropic",
        description="Endpoint discriminator resolved by the dispatcher.",
    )
    model_id: str = Field(
        ...,
        description="Provider-specific model identifier pinned by the dispatcher.",
    )
    parameters: dict = Field(
        default_factory=dict,
        description="Additional capability-resolved parameters forwarded to the router untouched.",
    )
    max_tokens: int = Field(default=4096, ge=256, le=64_000)

    # `model_id` collides with pydantic v2's reserved `model_` prefix.
    model_config = {"protected_namespaces": ()}
