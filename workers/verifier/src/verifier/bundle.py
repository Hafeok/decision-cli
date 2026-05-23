"""Parses an incoming verifier dispatch bundle into a VerifierInput struct."""

from __future__ import annotations

from pydantic import BaseModel, Field


class TcRef(BaseModel):
    """One test criterion the verifier must judge the artifact against."""

    id: str = Field(..., description="Test criterion identifier, e.g. 'TC-013'.")
    type: str = Field(
        default="",
        description="TC type label: invariant, exit-criteria, scenario, …",
    )
    body: str = Field(default="", description="Markdown body of the TC.")


class AdrRef(BaseModel):
    """One cross-cutting ADR the verifier should consult."""

    id: str = Field(..., description="ADR identifier, e.g. 'ADR-005'.")
    scope: str = Field(
        default="",
        description="ADR scope label (e.g. 'cross-cutting').",
    )
    body: str = Field(default="", description="Markdown body of the ADR.")


class VerifierInput(BaseModel):
    """Inputs the harness hands the verifier for a single interpretation.

    Matches the bundle FT-022's delivery handler assembles. The verifier
    treats the bundle as a complete, self-contained context: it does NOT
    fetch additional files, query the graph, or make follow-up calls
    (ADR-008 / ADR-020).
    """

    dispatch_id: str = Field(
        ...,
        description="IRI of the verifier dispatch event (PROV-O target).",
    )
    dispatch_group: str = Field(
        ...,
        description="IRI of the parent DispatchGroup (ADR-017).",
    )
    interpretation_session: str = Field(
        ...,
        description="IRI of the InterpretationSession produced by the harness.",
    )
    action_session: str = Field(
        ...,
        description="IRI of the paired ActionSession whose output is under review.",
    )
    feature_id: str = Field(
        ...,
        description="Feature being verified (e.g. 'FT-007').",
    )
    feature_spec: str = Field(
        ...,
        description="Full feature_spec markdown.",
    )
    produced_artifact: str = Field(
        ...,
        description="Text representation of the produced action artifact "
        "(typically the CodeChange diff or its summary).",
    )
    produced_artifact_iri: str = Field(
        default="",
        description="IRI of the produced artifact (typically the CodeChange).",
    )
    bundle_hash: str = Field(
        ...,
        description="SHA-256 hex of the action's input bundle (audit only).",
    )
    in_stream: str = Field(
        ...,
        description="ValueStream IRI (ADR-005) the verdict must carry.",
    )
    relevant_tcs: list[TcRef] = Field(
        default_factory=list,
        description="Test criteria the verifier must consider when judging "
        "the produced artifact.",
    )
    relevant_adrs: list[AdrRef] = Field(
        default_factory=list,
        description="Cross-cutting ADRs bounding the action.",
    )
    endpoint: str = Field(
        default="anthropic",
        description="Endpoint discriminator resolved by the dispatcher "
        "(FT-061): 'anthropic' or 'scaleway'.",
    )
    model_id: str = Field(
        ...,
        description="Provider-specific model identifier pinned by the "
        "dispatcher per FT-061. Workers do not hardcode this; it arrives "
        "in the dispatch payload alongside the endpoint discriminator.",
    )
    parameters: dict = Field(
        default_factory=dict,
        description="Additional capability-resolved parameters "
        "(reasoning_effort, exposes_reasoning_trace, …) forwarded to "
        "the router untouched.",
    )
    max_tokens: int = Field(default=4096, ge=256, le=64_000)

    # `model_id` collides with pydantic v2's reserved `model_` prefix.
    model_config = {"protected_namespaces": ()}

    @property
    def references(self) -> list[str]:
        """All TC and ADR identifiers in the bundle (for re-prompt hints)."""
        return [t.id for t in self.relevant_tcs] + [a.id for a in self.relevant_adrs]
