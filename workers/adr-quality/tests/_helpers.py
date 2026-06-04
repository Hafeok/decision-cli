"""Shared fixture helpers for adr-quality tests."""

from __future__ import annotations

from adr_quality.bundle import (
    AcknowledgementRecord,
    AdrProposalRecord,
    AdrQualityInput,
    AdrQualityRubricRecord,
    AdrSummaryRecord,
    AuthorityRecord,
    BodySchemaRecord,
    DomainRecord,
    GapRecord,
    NewAdrRecord,
    PreflightGapRecord,
)

CONFORMING_ADR_BODY = """## Status

Proposed.

## Context

The orchestration substrate currently lacks a documented policy for handling
worker timeouts. Without a policy, two failure modes emerge: (a) operators
silently raise the per-role budget every time a dispatch hangs, and (b)
ambient state about acceptable latency drifts away from the graph.

## Decision

Every role declares a `dec:roleTimeout` literal carrying the upper bound
in seconds. The harness terminates the worker subprocess on timeout and
emits a `dec:Feedback` artifact of class `defect` against the role.

## Rejected alternatives

- **No timeout at all.** Tolerable for local development; intolerable for a
  long-lived value stream. A wedged worker holds the dispatch lifecycle in
  `pending` forever, blocking the entire planner loop. Rejected.

- **Global timeout in `.dec/config.toml`.** A single number cannot serve a
  fast linter role and a slow code-writer role equally. Rejected; the
  per-role declaration in the catalog is the right shape.

## Consequences

Operators gain a uniform timeout vocabulary. Catalog seeds grow by one
literal per role. SHACL refuses any `dec:Role` without `dec:roleTimeout`.
"""

BARE_ALTERNATIVES_ADR_BODY = """## Status

Proposed.

## Context

A gap exists in the orchestration substrate.

## Decision

Add a thing.

## Rejected alternatives

## Consequences

Operators benefit.
"""


def build_bundle(
    *,
    bundle_hash: str = "adrbund1",
    proposal_iri: str = "urn:adr-proposal:FT-101",
    feature_id: str = "FT-101",
    feature_spec: str = "## Description\n\nA feature spec under review.\n",
    feature_spec_iri: str = "urn:feature-spec:FT-101",
    proposal_kind: str = "new",
    proposal_scope: str = "cross-cutting",
    proposal_body: str = CONFORMING_ADR_BODY,
    preflight_gap_kind: str = "unacknowledged-adr",
    preflight_gap_adr_id: str | None = "ADR-013",
    preflight_gap_domain: str | None = None,
    preflight_gap_iri: str = "urn:preflight-gap:adr:ADR-013",
    acknowledgement: AcknowledgementRecord | None = None,
    gap: GapRecord | None = None,
) -> AdrQualityInput:
    """Construct an AdrQualityInput for tests."""
    if proposal_kind == "new":
        proposal = AdrProposalRecord(
            iri=proposal_iri,
            kind="new",
            bundle_hash=bundle_hash,
            new=NewAdrRecord(
                title="ADR closing the gap",
                body=proposal_body,
                scope=proposal_scope,
                proposed_domains=["observability"],
                rationale=(
                    "Closes the preflight gap by explicitly governing worker "
                    "timeout policy across the orchestration substrate."
                ),
            ),
        )
    elif proposal_kind == "acknowledgement":
        proposal = AdrProposalRecord(
            iri=proposal_iri,
            kind="acknowledgement",
            bundle_hash=bundle_hash,
            acknowledgement=acknowledgement
            or AcknowledgementRecord(
                acknowledges="ADR-013",
                target_feature=feature_id,
                reasoning=(
                    "ADR-013 governs code-quality rules workspace-wide, including "
                    "the file-length and function-length limits this feature must "
                    "respect when it lands the new worker package."
                ),
                rationale=(
                    "Acknowledging ADR-013 satisfies the preflight gap without "
                    "authoring a new ADR; the existing one already governs this case."
                ),
            ),
        )
    else:
        proposal = AdrProposalRecord(
            iri=proposal_iri,
            kind="gap",
            bundle_hash=bundle_hash,
            gap=gap
            or GapRecord(
                missing_information=["scope", "boundary"],
                reason="Brief is too terse to support either path.",
            ),
        )

    preflight_gap = PreflightGapRecord(
        iri=preflight_gap_iri,
        kind=preflight_gap_kind,
        adr_id=preflight_gap_adr_id,
        domain=preflight_gap_domain,
        severity="warning",
        message="A cross-cutting concern is unacknowledged by this feature.",
    )

    return AdrQualityInput(
        adr_proposal=proposal,
        feature_id=feature_id,
        feature_spec=feature_spec,
        feature_spec_iri=feature_spec_iri,
        preflight_gap=preflight_gap,
        central_adrs=[
            AdrSummaryRecord(
                id="ADR-013",
                title="Code structure and quality standards",
                scope="cross-cutting",
                summary="Governs source-file and function length limits and module decomposition.",
            ),
            AdrSummaryRecord(
                id="ADR-073",
                title="Authoring roles as action-interpretation pairs",
                scope="cross-cutting",
                summary="Every authoring role pairs an author with a judge.",
            ),
            AdrSummaryRecord(
                id="ADR-074",
                title="QualityVerdict as a sibling type to VerificationVerdict",
                scope="cross-cutting",
                summary="Polymorphic verdict shape with judges + against fields.",
            ),
        ],
        adr_body_schema=BodySchemaRecord(
            required_h2_sections=[
                "Status",
                "Context",
                "Decision",
                "Rejected alternatives",
                "Consequences",
            ],
        ),
        domain_registry=[
            DomainRecord(name="api", description="CLI / MCP / worker contracts."),
            DomainRecord(name="observability", description="Tracing, telemetry."),
        ],
        rubric=AdrQualityRubricRecord(
            criteria=[
                "schema-conforming",
                "gap-closing",
                "scope-correct",
                "domain-valid",
                "alternatives-noted",
            ],
            description="Five-criterion rubric for new-ADR proposals.",
        ),
        authority=AuthorityRecord(
            may_decide=["phrasing", "section-order-within-h2"],
            must_escalate=["central-adr-changes"],
            rationale="adr-quality decides phrasing, escalates structural decisions.",
        ),
        bundle_hash=bundle_hash,
        model_id="claude-sonnet-4.5",
        endpoint="anthropic",
        parameters={},
        max_tokens=4096,
    )


def extract_bundle_hash(user_prompt: str) -> str:
    """Pull the bundle_hash echo line out of the rendered prompt."""
    marker = "**Bundle hash (echo this verbatim in your verdict)**:"
    for line in user_prompt.splitlines():
        if marker in line:
            return line.split(marker, 1)[1].strip()
    return ""
