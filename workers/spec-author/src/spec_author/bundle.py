"""Parses an incoming spec-author dispatch bundle into a SpecAuthorInput struct."""

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


class FeatureRecord(BaseModel):
    """One related feature_spec, summarised for the author's context."""

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


class AuthorityRecord(BaseModel):
    """The role's ADR-027 authority declaration.

    Names what the spec-author may decide unilaterally vs. what it must
    escalate via Gap when the brief is silent.
    """

    may_decide: list[str] = Field(
        default_factory=list,
        description="Categories the spec-author may resolve from the brief alone.",
    )
    must_escalate: list[str] = Field(
        default_factory=list,
        description="Categories the spec-author MUST emit Gap on rather than invent.",
    )
    rationale: str = Field(
        default="",
        description="Why this scope is correct (read by humans; ignored by the worker).",
    )


class SpecAuthorInput(BaseModel):
    """Inputs the harness hands the spec-author for a single proposal.

    Matches the bundle FT-129's spec defines. The worker treats the
    bundle as a complete, self-contained context: it does NOT fetch
    additional files, query the graph, or make follow-up calls
    (ADR-008 / ADR-073).
    """

    request: RequestRecord = Field(..., description="The originating brief / request.")
    feature_id_hint: Optional[str] = Field(
        default=None,
        description="Placeholder FT-NNN if one has been minted upstream.",
    )
    body_schema: BodySchemaRecord = Field(
        ...,
        description="H2/H3 contract from ADR-047 and product.toml [features].",
    )
    related_features: list[FeatureRecord] = Field(
        default_factory=list,
        description="Nearby specs by domain or depends-on relationship.",
    )
    central_adrs: list[AdrSummaryRecord] = Field(
        default_factory=list,
        description="Graph-central + cross-cutting ADRs the spec must respect.",
    )
    domain_registry: list[DomainRecord] = Field(
        default_factory=list,
        description="Allowed domain values from product.toml.",
    )
    authority: AuthorityRecord = Field(
        ...,
        description="ADR-027 authority declaration for the spec-author role.",
    )
    bundle_hash: str = Field(
        ...,
        min_length=8,
        description="SHA-256 hex of the canonical bundle (echoed in the proposal).",
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
    max_tokens: int = Field(default=8192, ge=256, le=64_000)

    # `model_id` collides with pydantic v2's reserved `model_` prefix.
    model_config = {"protected_namespaces": ()}

    @property
    def related_feature_ids(self) -> list[str]:
        """Identifiers of every related feature in the bundle."""
        return [f.id for f in self.related_features]
