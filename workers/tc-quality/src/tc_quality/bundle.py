"""Parses an incoming tc-quality dispatch bundle into a TcQualityInput struct."""

from __future__ import annotations

from pydantic import BaseModel, Field


class TcRecord(BaseModel):
    """One existing test criterion linked to the feature."""

    id: str = Field(..., description="Test criterion identifier, e.g. 'TC-013'.")
    title: str = Field(default="", description="Short TC title.")
    status: str = Field(default="", description="TC status (draft, active, superseded).")
    runner: str = Field(default="", description="Runner kind (bash, cargo-test, pytest, custom).")
    runner_args: str = Field(default="", description="Args the runner takes.")
    runner_timeout: str = Field(default="", description="Timeout for this runner.")
    body: str = Field(default="", description="Full markdown body of the TC.")


class ProposedTcRecord(BaseModel):
    """One proposed TC from a TcProposal (the artifact being judged)."""

    id: str = Field(..., description="Proposed TC identifier.")
    title: str = Field(..., description="Short TC title.")
    body: str = Field(..., description="Full TC body (acceptance criteria).")
    runner: str = Field(..., description="Runner kind.")
    runner_args: str = Field(..., description="Args the runner takes.")
    runner_timeout: str = Field(default="60s", description="Timeout for this runner.")
    observes: list[str] = Field(
        default_factory=list,
        description="Coverage axes observed by this TC.",
    )


class ProposedNewRecord(BaseModel):
    """Shape for kind='new' proposals."""

    tcs: list[ProposedTcRecord] = Field(
        ...,
        description="List of proposed TCs.",
    )


class ProposedAugmentRecord(BaseModel):
    """Shape for kind='augment' proposals."""

    additions: list[ProposedTcRecord] = Field(
        ...,
        description="Additional TCs to add.",
    )


class ProposedSufficientRecord(BaseModel):
    """Shape for kind='sufficient' proposals."""

    reasoning: str = Field(..., description="Why existing TCs are sufficient.")
    coverage_map: dict[str, list[str]] = Field(
        default_factory=dict,
        description="Map of coverage axes to TC IDs that cover them.",
    )


class TcProposalRecord(BaseModel):
    """The TcProposal artifact being judged (output of tc-author / FT-126)."""

    kind: str = Field(
        ...,
        description="Proposal kind: 'new', 'augment', or 'sufficient'.",
    )
    bundle_hash: str = Field(..., description="Echoed bundle hash.")
    new: ProposedNewRecord | None = Field(None, description="Present when kind='new'.")
    augment: ProposedAugmentRecord | None = Field(None, description="Present when kind='augment'.")
    sufficient: ProposedSufficientRecord | None = Field(
        None, description="Present when kind='sufficient'."
    )


class TcQualityRubricRecord(BaseModel):
    """The five criteria tc-quality scores against (FT-127)."""

    criteria: list[str] = Field(
        ...,
        description="List of criterion names: clear, testable, non-redundant, faithful, runner-wireable.",
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


class TcQualityInput(BaseModel):
    """Inputs the harness hands the tc-quality judge for a single verdict.

    Matches the bundle FT-127's spec defines. The worker treats the
    bundle as a complete, self-contained context: it does NOT fetch
    additional files, query the graph, or make follow-up calls
    (ADR-008 / ADR-073).
    """

    feature_id: str = Field(..., description="Feature the TCs serve (e.g. 'FT-007').")
    feature_spec: str = Field(..., description="Full feature_spec markdown body.")
    tc_proposal: TcProposalRecord = Field(
        ...,
        description="The TcProposal artifact under judgment.",
    )
    existing_tcs: list[TcRecord] = Field(
        default_factory=list,
        description="TCs already linked to the feature (for non-redundancy check).",
    )
    rubric: TcQualityRubricRecord = Field(
        ...,
        description="The five criteria tc-quality scores against.",
    )
    authority: AuthorityRecord = Field(
        ...,
        description="Authority declaration per ADR-027.",
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
