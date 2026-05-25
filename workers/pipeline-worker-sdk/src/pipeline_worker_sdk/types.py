"""Pydantic models for the dispatch/completion wire protocol (FT-077)."""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel, Field


class DispatchEvent(BaseModel):
    """A single dispatch envelope parsed off the SSE stream.

    Per ADR-045 (SSE for dispatches) the harness frames each dispatch as one
    SSE record with `id:` (the monotonic seq used for Last-Event-ID resume),
    `event:` (always ``dispatch`` for slice 1), and `data:` (a JSON envelope
    that — per ADR-046 — embeds an N-Quads payload string under
    ``nquads_payload``). The SDK does not parse the N-Quads here; it hands
    them up to the Session layer (FT-078) for pyoxigraph ingest.
    """

    event_id: str = Field(
        ...,
        description="The SSE `id:` field, used as Last-Event-ID on reconnect.",
    )
    dispatch_id: str = Field(
        ...,
        description="Stable IRI of the dec:Dispatch artifact this event references.",
    )
    capability_tag: str = Field(
        ...,
        description="The capability tag this dispatch is bound to (ADR-047).",
    )
    nquads_payload: str = Field(
        default="",
        description="N-Quads body of the dispatch bundle (ADR-046).",
    )
    metadata: dict[str, Any] = Field(
        default_factory=dict,
        description="Free-form envelope metadata (role_id, bundle_hash, …).",
    )


class CompletionPayload(BaseModel):
    """The body of a worker → harness completion POST (ADR-045).

    The worker hands the harness an N-Quads payload describing the produced
    artifact plus session-side telemetry. The harness validates via SHACL at
    the GraphWriter chokepoint (ADR-041) and returns 200/2xx on accept or
    4xx with a validation report on reject.
    """

    dispatch_id: str = Field(
        ..., description="The dispatch this completion is responding to."
    )
    session_id: str = Field(
        ...,
        description="The SDK-side Session URI (FT-078) — same identity as the "
        "harness PROV-O Activity (ADR-050).",
    )
    nquads_payload: str = Field(
        default="",
        description="N-Quads body of the produced artifact + session telemetry.",
    )
    outcome: str = Field(
        default="success",
        description="Coarse outcome tag: success | failed | feedback-only.",
    )
    telemetry: dict[str, Any] = Field(
        default_factory=dict,
        description="Free-form session telemetry (tokens used, cost, latency).",
    )


class ClaimResult(BaseModel):
    """The outcome of an atomic claim request on a dispatch.

    Multiple workers may receive the same SSE dispatch envelope if they share
    a capability tag; the claim endpoint resolves the contention by returning
    ``won=True`` to exactly one caller. Losers see ``won=False`` and skip the
    dispatch without producing a completion.
    """

    dispatch_id: str
    won: bool
    reason: str = Field(
        default="",
        description="On loss, the harness's explanation (e.g. 'already-claimed').",
    )


class CapabilityCatalogEntry(BaseModel):
    """A single capability-tag → model-group mapping from the harness catalog.

    Used by `CatalogCache` (FT-077) so the worker does not re-fetch the same
    mapping on every dispatch. Capability bindings change orders of magnitude
    less often than dispatches (ADR-033).
    """

    capability_tag: str
    model_group: str = Field(
        ...,
        description="The LiteLLM model group the capability tag resolves to (ADR-054).",
    )
    endpoint: str = Field(
        default="",
        description="The provider endpoint (anthropic, scaleway, …) for audit.",
    )
    version: str = Field(
        default="",
        description="Catalog entry version pin (semver or content hash).",
    )
