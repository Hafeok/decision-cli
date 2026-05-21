"""Parses an incoming verify-graph-author dispatch bundle into a VerifyGraphAuthorInput struct."""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel, Field


class TcRecord(BaseModel):
    """One test criterion the proposed graph must produce evidence for."""

    id: str = Field(..., description="Test criterion identifier, e.g. 'TC-013'.")
    title: str = Field(default="", description="Short TC title for the prompt.")
    body: str = Field(default="", description="Markdown body of the TC.")


class EnvRecord(BaseModel):
    """The target verification environment for this proposal."""

    id: str = Field(..., description="Environment identifier, e.g. 'ENV-1'.")
    env_type: str = Field(
        ...,
        description="Environment kind label (e.g. 'ephemeral-tempdir', 'http-endpoint').",
    )
    safety_class: str = Field(
        default="",
        description="Safety classification (e.g. 'read-only', 'mutating').",
    )
    allowed_ops: list[str] = Field(
        default_factory=list,
        description="Operations the environment permits. Steps requiring ops "
        "not in this list must NOT appear in a New proposal.",
    )
    endpoint: str | None = Field(
        default=None,
        description="Optional endpoint URL when relevant (e.g. for http-* envs).",
    )


class StepSummary(BaseModel):
    """A condensed view of one step in an existing graph (for the prompt)."""

    step_type: str = Field(..., description="Step kind (e.g. 'shell-command').")
    summary: str = Field(default="", description="Short description of what the step does.")
    provides_evidence_for: list[str] = Field(
        default_factory=list,
        description="TC ids this step provides evidence for in the existing graph.",
    )


class ExistingGraphRecord(BaseModel):
    """One existing verification graph the worker may match against."""

    id: str = Field(..., description="Existing graph identifier, e.g. 'VG-007'.")
    verifies: str = Field(
        default="",
        description="Feature this graph verifies (may be the same as the bundle's feature).",
    )
    covers: list[str] = Field(
        default_factory=list,
        description="TC ids this existing graph currently covers in the target env.",
    )
    step_summaries: list[StepSummary] = Field(
        default_factory=list,
        description="Ordered, condensed list of the graph's steps.",
    )


class StepKindRecord(BaseModel):
    """One available step kind in the worker's vocabulary."""

    kind: str = Field(..., description="Step kind name (one of the six seed kinds).")
    required_ops: list[str] = Field(
        default_factory=list,
        description="Operations this kind needs at execution time; must be a "
        "subset of `target_environment.allowed_ops` for a step of this kind "
        "to be valid in the target environment.",
    )
    fields_schema: dict[str, Any] = Field(
        default_factory=dict,
        description="JSON-schema-shaped contract for the step's `fields` payload.",
    )
    description: str = Field(default="", description="Human-readable purpose of the kind.")


class VerifyGraphAuthorInput(BaseModel):
    """Inputs the harness hands the verify-graph-author for a single proposal.

    Matches the bundle FT-049's assembler builds. The worker treats the
    bundle as a complete, self-contained context: it does NOT fetch
    additional files, query the graph, or make follow-up calls
    (ADR-008 / ADR-030).
    """

    feature_id: str = Field(..., description="Feature being verified (e.g. 'FT-007').")
    feature_spec: str = Field(..., description="Full feature_spec markdown body.")
    relevant_tcs: list[TcRecord] = Field(
        default_factory=list,
        description="Test criteria the proposed graph must cover.",
    )
    target_environment: EnvRecord = Field(
        ...,
        description="Exactly one target environment per call (ADR-030).",
    )
    candidate_graphs: list[ExistingGraphRecord] = Field(
        default_factory=list,
        description="Existing graphs that touch any of the feature's TCs in this env.",
    )
    step_vocabulary: list[StepKindRecord] = Field(
        default_factory=list,
        description="Step kinds the worker is allowed to propose.",
    )
    bundle_hash: str = Field(
        ...,
        min_length=8,
        description="SHA-256 hex of the canonical bundle (echoed in the proposal).",
    )
    model_id: str = Field(
        default="claude-sonnet-4-5",
        description="Model the worker should invoke (single hardcoded binding per ADR-020).",
    )
    max_tokens: int = Field(default=4096, ge=256, le=64_000)

    # `model_id` collides with pydantic v2's reserved `model_` prefix.
    model_config = {"protected_namespaces": ()}

    @property
    def tc_ids(self) -> list[str]:
        """Identifiers of every TC the proposal must address."""
        return [t.id for t in self.relevant_tcs]
