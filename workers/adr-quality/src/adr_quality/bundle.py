"""Parses an incoming adr-quality dispatch bundle into an AdrQualityInput struct."""

from __future__ import annotations

from typing import Literal, Optional

from pydantic import BaseModel, Field


class PreflightGapRecord(BaseModel):
    """The preflight diagnostic the AdrProposal claims to address.

    Mirrors the gap shape from FT-130 (`product preflight` surfaces):
    either an unacknowledged cross-cutting ADR or an uncovered domain.
    The judge consults this to score the gap-closing and scope-correct
    criteria.
    """

    iri: str = Field(
        default="",
        description="Stable IRI / identifier of the preflight gap (for the verdict's `against`).",
    )
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


class NewAdrRecord(BaseModel):
    """The new-kind payload mirroring adr-author's NewAdrProposal."""

    title: str = Field(default="", description="Title of the proposed ADR.")
    body: str = Field(default="", description="Full markdown body of the ADR proposal.")
    scope: str = Field(
        default="",
        description=(
            "ADR scope ('cross-cutting', 'platform', 'domain', 'feature-specific')."
        ),
    )
    proposed_domains: list[str] = Field(
        default_factory=list,
        description="Domain names from domain_registry the ADR belongs to.",
    )
    rationale: str = Field(
        default="",
        description="Author's rationale for why the ADR addresses the gap.",
    )


class AcknowledgementRecord(BaseModel):
    """The acknowledgement-kind payload mirroring adr-author's AcknowledgementProposal.

    Brief §4B: bare acknowledgements (empty `reasoning`) are rejected at
    the action worker; the judge applies the same rule as defense in depth.
    """

    acknowledges: str = Field(
        default="",
        description="ADR identifier (e.g. 'ADR-013') or domain name being acknowledged.",
    )
    target_feature: str = Field(
        default="",
        description="Feature this acknowledgement applies to (e.g. 'FT-101').",
    )
    reasoning: str = Field(
        default="",
        description=(
            "Substantive reasoning explaining why the acknowledged ADR/domain "
            "governs the feature. MUST be >= 40 chars per FT-130 §4B."
        ),
    )
    rationale: str = Field(
        default="",
        description="Why this acknowledgement closes the preflight gap.",
    )


class GapRecord(BaseModel):
    """The gap-kind payload mirroring adr-author's GapProposal."""

    missing_information: list[str] = Field(
        default_factory=list,
        description="Concrete items the brief should clarify before authoring is possible.",
    )
    reason: str = Field(
        default="",
        description="Why the author could not produce a defensible ADR or acknowledgement.",
    )


class AdrProposalRecord(BaseModel):
    """The AdrProposal artifact being judged (output of adr-author / FT-130)."""

    iri: str = Field(
        default="",
        description="IRI / identifier of the AdrProposal under judgment.",
    )
    kind: Literal["new", "acknowledgement", "gap"] = Field(
        ...,
        description="Proposal kind discriminator: 'new', 'acknowledgement', or 'gap'.",
    )
    bundle_hash: str = Field(default="", description="Echoed bundle hash from adr-author.")
    new: Optional[NewAdrRecord] = Field(
        default=None,
        description="Present when kind='new'.",
    )
    acknowledgement: Optional[AcknowledgementRecord] = Field(
        default=None,
        description="Present when kind='acknowledgement'.",
    )
    gap: Optional[GapRecord] = Field(
        default=None,
        description="Present when kind='gap'.",
    )


class AdrSummaryRecord(BaseModel):
    """One graph-central or cross-cutting ADR digest the judge may consult."""

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

    ADRs use a five-section H2 layout (Context, Decision, Rejected
    alternatives, Consequences, Status). The judge enforces the
    schema-conforming criterion against this list.
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


class AdrQualityRubricRecord(BaseModel):
    """The criteria adr-quality scores against (FT-133)."""

    criteria: list[str] = Field(
        ...,
        description=(
            "List of criterion names. For new-ADR proposals: "
            "['schema-conforming', 'gap-closing', 'scope-correct', "
            "'domain-valid', 'alternatives-noted']. For acknowledgement "
            "proposals: ['reasoning-length-floor', 'references-existing', "
            "'reasoning-relevance', 'gap-matching', 'not-better-as-new']."
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


class AdrQualityInput(BaseModel):
    """Inputs the harness hands the adr-quality judge for a single verdict.

    Matches the bundle FT-133's spec defines. The worker treats the
    bundle as a complete, self-contained context: it does NOT fetch
    additional files, query the graph, or make follow-up calls
    (ADR-008 / ADR-073).
    """

    adr_proposal: AdrProposalRecord = Field(
        ...,
        description="The AdrProposal artifact under judgment (output of FT-130).",
    )
    feature_id: str = Field(
        ...,
        description="Feature whose preflight surfaced the gap (e.g. 'FT-101').",
    )
    feature_spec: str = Field(
        default="",
        description="Full markdown body of the feature_spec that surfaced the gap.",
    )
    feature_spec_iri: str = Field(
        default="",
        description=(
            "Stable IRI of the feature_spec (for the verdict's `against`). "
            "Falls back to `urn:feature-spec:<feature_id>` if unspecified."
        ),
    )
    preflight_gap: PreflightGapRecord = Field(
        ...,
        description="The gap the proposal addresses (also a `dec:against` referent).",
    )
    central_adrs: list[AdrSummaryRecord] = Field(
        default_factory=list,
        description="Graph-central + cross-cutting ADRs the proposal must respect.",
    )
    adr_body_schema: BodySchemaRecord = Field(
        ...,
        description="H2 contract for ADRs (echoed in the prompt and used for validation).",
    )
    domain_registry: list[DomainRecord] = Field(
        default_factory=list,
        description="Allowed domain values from product.toml.",
    )
    rubric: AdrQualityRubricRecord = Field(
        ...,
        description="The criteria adr-quality scores against.",
    )
    authority: AuthorityRecord = Field(
        ...,
        description="ADR-027 authority declaration for the adr-quality role.",
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

    @property
    def central_adr_ids(self) -> list[str]:
        """Identifiers of every central ADR carried in the bundle."""
        return [a.id for a in self.central_adrs]

    @property
    def preflight_gap_iri(self) -> str:
        """Resolve the gap IRI for use in `against` (falls back to a URN)."""
        if self.preflight_gap.iri:
            return self.preflight_gap.iri
        if self.preflight_gap.kind == "unacknowledged-adr" and self.preflight_gap.adr_id:
            return f"urn:preflight-gap:adr:{self.preflight_gap.adr_id}"
        if self.preflight_gap.kind == "uncovered-domain" and self.preflight_gap.domain:
            return f"urn:preflight-gap:domain:{self.preflight_gap.domain}"
        return f"urn:preflight-gap:{self.feature_id}"

    @property
    def resolved_feature_spec_iri(self) -> str:
        """Resolve the feature_spec IRI for use in `against` (falls back to a URN)."""
        return self.feature_spec_iri or f"urn:feature-spec:{self.feature_id}"
