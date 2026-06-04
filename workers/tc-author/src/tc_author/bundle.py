"""Parses an incoming tc-author dispatch bundle into a TcAuthorInput struct."""

from __future__ import annotations

from pydantic import BaseModel, Field


class TcRecord(BaseModel):
    """One existing test criterion linked to the feature."""

    id: str = Field(..., description="Test criterion identifier, e.g. 'TC-013'.")
    title: str = Field(default="", description="Short TC title.")
    status: str = Field(default="", description="TC status (draft, active, superseded).")
    runner: str = Field(default="", description="Runner kind (bash, cargo-test, pytest, custom).")
    runner_args: str = Field(default="", description="Args the runner takes.")
    body: str = Field(default="", description="Full markdown body of the TC.")


class TcSchemaRecord(BaseModel):
    """Field cardinalities and allowed runners for TCs."""

    required_fields: list[str] = Field(
        default_factory=list,
        description="Required frontmatter fields for a TC.",
    )
    allowed_types: list[str] = Field(
        default_factory=list,
        description="Allowed TC types (scenario, exit-criteria, invariant).",
    )
    allowed_runners: list[str] = Field(
        default_factory=list,
        description="Allowed runner kinds (bash, cargo-test, pytest, custom).",
    )


class RunnerKindRecord(BaseModel):
    """One runner kind in the controlled vocabulary."""

    kind: str = Field(..., description="Runner kind name (bash, cargo-test, pytest, custom).")
    args_pattern: str = Field(
        default="",
        description="Regex pattern runner_args must match for this kind.",
    )
    timeout_default: str = Field(
        default="60s",
        description="Default timeout for this runner kind.",
    )


class CoverageAxisRecord(BaseModel):
    """One coverage axis from ADR-072."""

    axis: str = Field(..., description="Axis name (happy, edge, integration, state).")
    description: str = Field(
        default="",
        description="What this axis covers.",
    )


class TcAuthorInput(BaseModel):
    """Inputs the harness hands the tc-author for a single proposal.

    Matches the bundle FT-126's spec defines. The worker treats the
    bundle as a complete, self-contained context: it does NOT fetch
    additional files, query the graph, or make follow-up calls
    (ADR-008 / ADR-073).
    """

    feature_id: str = Field(..., description="Feature being authored TCs for (e.g. 'FT-007').")
    feature_spec: str = Field(..., description="Full feature_spec markdown body.")
    existing_tcs: list[TcRecord] = Field(
        default_factory=list,
        description="TCs already linked to the feature.",
    )
    tc_schema: TcSchemaRecord = Field(
        ...,
        description="TC frontmatter contract (allowed types, required fields, allowed runners).",
    )
    runner_vocabulary: list[RunnerKindRecord] = Field(
        default_factory=list,
        description="Controlled vocabulary of runner kinds.",
    )
    target_count: int = Field(
        ...,
        ge=1,
        description="min_tcs_per_feature from ADR-072 — the floor the feature must meet.",
    )
    coverage_axes: list[CoverageAxisRecord] = Field(
        default_factory=list,
        description="Advisory coverage axes (happy, edge, integration, state) from ADR-072.",
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
    max_tokens: int = Field(default=4096, ge=256, le=64_000)

    # `model_id` collides with pydantic v2's reserved `model_` prefix.
    model_config = {"protected_namespaces": ()}

    @property
    def tc_ids(self) -> list[str]:
        """Identifiers of every existing TC."""
        return [t.id for t in self.existing_tcs]
