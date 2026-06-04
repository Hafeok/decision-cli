"""Serialises a validated TcProposal to the harness JSON shape."""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field


ProposalKind = Literal["sufficient", "augment", "new"]


class SufficientProposal(BaseModel):
    """The existing TCs already meet target_count; no new TCs needed."""

    model_config = ConfigDict(extra="forbid")

    reasoning: str = Field(
        ...,
        min_length=20,
        description="Why existing TCs already meet target_count.",
    )
    coverage_map: dict[str, list[str]] = Field(
        ...,
        description="TC-id -> coverage_axes hit (shows how existing TCs cover axes).",
    )


class ProposedTc(BaseModel):
    """One TC the worker proposes to add."""

    model_config = ConfigDict(extra="forbid")

    title: str = Field(..., min_length=1, description="TC title.")
    type: Literal["scenario", "exit-criteria", "invariant"] = Field(
        ...,
        description="TC type from the TC schema.",
    )
    body: str = Field(
        ...,
        min_length=1,
        description="Full markdown body with H2 sections per TC schema.",
    )
    runner: Literal["bash", "cargo-test", "pytest", "custom"] = Field(
        ...,
        description="Runner kind from the runner vocabulary.",
    )
    runner_args: str = Field(
        ...,
        min_length=1,
        description="Args for the runner — REQUIRED and non-empty per FT-126 invariants.",
    )
    runner_timeout: str = Field(
        default="60s",
        description="Timeout for this TC (e.g. '60s').",
    )
    observes: list[str] = Field(
        ...,
        min_length=1,
        description="FT-072 observed surfaces — at least one required.",
    )
    coverage_axis: str | None = Field(
        default=None,
        description="Advisory tag from coverage_axes list (happy, edge, integration, state).",
    )
    validates_features: list[str] = Field(
        ...,
        min_length=1,
        description="Feature IDs this TC validates — always includes feature_id.",
    )
    validates_adrs: list[str] = Field(
        default_factory=list,
        description="Cross-cutting ADRs cited in the spec.",
    )


class AugmentProposal(BaseModel):
    """The worker proposes adding TCs to the existing partial coverage."""

    model_config = ConfigDict(extra="forbid")

    retained_tcs: list[str] = Field(
        default_factory=list,
        description="TC-ids kept as-is.",
    )
    additions: list[ProposedTc] = Field(
        ...,
        min_length=1,
        description="New TCs to add.",
    )
    reasoning: str = Field(
        ...,
        min_length=20,
        description="Why these additions bring coverage to target_count.",
    )


class NewProposal(BaseModel):
    """The worker proposes a fresh TC set."""

    model_config = ConfigDict(extra="forbid")

    replaced_tcs: list[str] = Field(
        default_factory=list,
        description="TC-ids the harness should supersede.",
    )
    tcs: list[ProposedTc] = Field(
        ...,
        min_length=1,
        description="Ordered, target_count entries.",
    )
    reasoning: str = Field(
        ...,
        min_length=20,
        description="Why this TC set covers the feature.",
    )


class TcProposal(BaseModel):
    """Single artifact returned by the tc-author worker (ADR-073).

    Exactly one of `sufficient` / `augment` / `new` is populated; `kind` is the
    discriminator. `bundle_hash` echoes the input hash so the harness can
    verify provenance (per ADR-073).
    """

    model_config = ConfigDict(extra="forbid")

    kind: ProposalKind = Field(..., description="Proposal kind discriminator.")
    bundle_hash: str = Field(
        ...,
        min_length=8,
        description="Echo of input bundle_hash for verification.",
    )
    sufficient: SufficientProposal | None = Field(
        default=None,
        description="Populated when kind='sufficient'.",
    )
    augment: AugmentProposal | None = Field(
        default=None,
        description="Populated when kind='augment'.",
    )
    new: NewProposal | None = Field(
        default=None,
        description="Populated when kind='new'.",
    )
