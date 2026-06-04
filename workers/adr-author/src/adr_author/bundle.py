"""Parses an incoming adr-author dispatch bundle into an AdrAuthorInput struct."""

from __future__ import annotations

from typing import Literal, Optional

from pydantic import BaseModel, Field


class PreflightGapRecord(BaseModel):
    """The preflight diagnostic this dispatch must address.

    Mirrors the gap shape `product preflight` surfaces — either an
    unacknowledged cross-cutting ADR or an uncovered domain.
    """

    kind: Literal["unacknowledged-adr", "uncovered-domain"] = Field(
        ...,
        description="Discriminator: ADR-shaped gap vs domain-shaped gap.",
    )
    adr_id: Optional[str] = Field(
        default=None,
        description="ADR identifier for kind='unacknowledged-adr' (e.g. 'ADR-013').",
    )
    domain: Optional[str] = Field(
        default=None,
        description="Domain name for kind='uncovered-domain'.",
    )
    severity: Literal["warning", "error"] = Field(
        ...,
        description="Severity surfaced by `product preflight`.",
    )
    message: str = Field(
        default="",
        description="Diagnostic message from `product preflight`.",
    )


class AdrSummaryRecord(BaseModel):
    """One graph-central or cross-cutting ADR digest the author may reference."""

    id: str = Field(..., description="ADR identifier (e.g. 'ADR-013').")
    title: str = Field(default="", description="ADR title.")
    scope: str = Field(
        default="",
        description="ADR scope ('cross-cutting', 'platform', 'domain', 'feature-specific').",
    )
    summary: str = Field(
        default="",
        description="One-paragraph digest of the ADR's Decision section.",
    )


class BodySchemaRecord(BaseModel):
    """The H2 contract for ADR bodies.

    ADRs in this repo use a stable five-section H2 layout (Context,
    Decision, Rejected alternatives, Consequences, Status). The bundle
    carries the required list so the prompt can echo it verbatim and
    the worker can validate before emit.
    """

    required_h2_sections: list[str] = Field(
        default_factory=list,
        description=(
            "Required top-level H2 sections (e.g. "
            "['Context', 'Decision', 'Rejected alternatives', 'Consequences', 'Status'])."
        ),
    )
    section_descriptions: dict[str, str] = Field(
        default_factory=dict,
        description="Optional per-section guidance (section name -> short description).",
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

    Names what the adr-author may decide unilaterally vs. what it must
    escalate via Gap when the brief is silent. Bare acknowledgement is
    forbidden regardless of authority — that is a separate boundary
    invariant enforced inside the worker (see FT-130 §Behaviour).
    """

    may_decide: list[str] = Field(
        default_factory=list,
        description="Categories the adr-author may resolve from the bundle alone.",
    )
    must_escalate: list[str] = Field(
        default_factory=list,
        description="Categories the adr-author MUST emit Gap on rather than invent.",
    )
    rationale: str = Field(
        default="",
        description="Why this scope is correct (read by humans; ignored by the worker).",
    )


class AdrAuthorInput(BaseModel):
    """Inputs the harness hands the adr-author for a single proposal.

    Matches the bundle FT-130's spec defines. The worker treats the
    bundle as a complete, self-contained context: it does NOT fetch
    additional files, query the graph, or make follow-up calls
    (ADR-008 / ADR-073).
    """

    feature_id: str = Field(
        ...,
        description="Feature whose preflight surfaced the gap (e.g. 'FT-101').",
    )
    feature_spec: str = Field(
        default="",
        description="Full markdown body of the feature_spec that surfaced the gap.",
    )
    preflight_gap: PreflightGapRecord = Field(
        ...,
        description="The gap this dispatch must address.",
    )
    central_adrs: list[AdrSummaryRecord] = Field(
        default_factory=list,
        description="Graph-central + cross-cutting ADRs the author may reference.",
    )
    adr_body_schema: BodySchemaRecord = Field(
        ...,
        description="H2 layout for ADRs (echoed in the prompt and used for validation).",
    )
    domain_registry: list[DomainRecord] = Field(
        default_factory=list,
        description="Allowed domain values from product.toml.",
    )
    authority: AuthorityRecord = Field(
        ...,
        description="ADR-027 authority declaration for the adr-author role.",
    )
    bundle_hash: str = Field(
        ...,
        min_length=8,
        description="SHA-256 hex (or hash-like) of the canonical bundle (echoed in the proposal).",
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
    def central_adr_ids(self) -> list[str]:
        """Identifiers of every central ADR carried in the bundle."""
        return [a.id for a in self.central_adrs]
