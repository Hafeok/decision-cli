"""Serialises a validated GraphProposal to the harness JSON shape."""

from __future__ import annotations

from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field, model_validator

ProposalKind = Literal["match", "new", "gap"]

# The six seed step kinds enumerated by ADR-030 / FT-036.
StepType = Literal[
    "shell-command",
    "sparql-assertion",
    "file-assertion",
    "http-request",
    "wait-for",
    "capture",
]


class MatchProposal(BaseModel):
    """The worker matched the feature to an existing graph; no new graph needed."""

    model_config = ConfigDict(extra="forbid")

    graph_id: str = Field(..., description="IRI / identifier of the matched existing graph.")
    rationale: str = Field(
        ...,
        min_length=1,
        description="Why this graph already covers the feature's TCs in this env.",
    )


class ProposedStep(BaseModel):
    """One step in a New proposal — kind, fields payload, and TC coverage."""

    model_config = ConfigDict(extra="forbid")

    step_type: StepType = Field(..., description="One of the six seed step kinds.")
    fields: dict[str, Any] = Field(
        default_factory=dict,
        description="Per-kind payload validated against the kind's fields_schema "
        "client-side after the worker returns.",
    )
    provides_evidence_for: list[str] = Field(
        ...,
        description="TC ids this step provides evidence for. An empty list is "
        "an explicit 'setup or capture' marker; the rationale must justify it.",
    )


class NewProposal(BaseModel):
    """The worker proposes a fresh graph in the target environment."""

    model_config = ConfigDict(extra="forbid")

    environment: str = Field(..., description="Target environment IRI / identifier.")
    steps: list[ProposedStep] = Field(
        ...,
        min_length=1,
        description="Ordered list of proposed steps; at least one is required.",
    )
    rationale: str = Field(
        ...,
        min_length=1,
        description="Why this step sequence covers the feature's TCs.",
    )


class GapProposal(BaseModel):
    """The worker cannot honestly produce a covering graph; reason in `reason`."""

    model_config = ConfigDict(extra="forbid")

    uncovered_tcs: list[str] = Field(
        ...,
        min_length=1,
        description="TC ids the worker cannot cover with the available vocabulary "
        "and allowed_ops.",
    )
    reason: str = Field(
        ...,
        min_length=1,
        description="Why the step vocabulary or environment is insufficient.",
    )


class GraphProposal(BaseModel):
    """Single artifact returned by the verify-graph-author worker (ADR-030).

    Exactly one of `match` / `new` / `gap` is populated; `kind` is the
    discriminator. `bundle_hash` echoes the input hash so the harness can
    verify the proposal pairs to the bundle it sent (FT-048 §Error 5).
    """

    # Refuse unknown / extra fields — ADR-008 worker contract demands a
    # strict shape so the harness can validate the JSON deterministically.
    model_config = ConfigDict(extra="forbid")

    kind: ProposalKind = Field(..., description="Discriminator: match / new / gap.")
    bundle_hash: str = Field(
        ...,
        min_length=8,
        description="Echo of the input bundle_hash for protocol integrity.",
    )
    match: MatchProposal | None = None
    new: NewProposal | None = None
    gap: GapProposal | None = None

    @model_validator(mode="after")
    def _exactly_one_payload(self) -> "GraphProposal":
        populated: list[str] = []
        if self.match is not None:
            populated.append("match")
        if self.new is not None:
            populated.append("new")
        if self.gap is not None:
            populated.append("gap")

        if len(populated) != 1:
            raise ValueError(
                f"exactly one of (match, new, gap) must be populated; got {populated or 'none'}"
            )
        if populated[0] != self.kind:
            raise ValueError(
                f"kind='{self.kind}' but populated payload is '{populated[0]}' "
                "(kind must match the populated payload)"
            )
        return self
