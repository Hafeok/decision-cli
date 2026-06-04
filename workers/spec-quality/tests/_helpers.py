"""Shared fixture helpers for spec-quality tests."""

from __future__ import annotations

from spec_quality.bundle import (
    AdrSummaryRecord,
    AuthorityRecord,
    BodySchemaRecord,
    DomainRecord,
    FeatureRecord,
    GapSpecRecord,
    NewSpecRecord,
    RequestRecord,
    SpecProposalRecord,
    SpecQualityInput,
    SpecQualityRubricRecord,
)

CONFORMING_BODY = """## Description

Worker spec authored for testing. It exercises the spec-quality judge.

## Functional Specification

### Inputs

A bundle with a SpecProposal and an originating request.

### Outputs

A QualityVerdict serialised as JSON on stdout.

### State

Stateless. No filesystem writes beyond stdout.

### Behaviour

Render the rubric prompt, call Claude, validate the response, echo bundle_hash.

### Invariants

Bundle hash on the verdict equals bundle hash on the input.

### Error handling

Exit 2 on malformed input, 3 on model failure, 4 on schema failure, 5 on hash mismatch.

### Boundaries

In scope: worker package; out of scope: persistence.

## Out of scope

- Persistence of the verdict.
- Multi-proposal batch verdicts.
- Auto-acceptance of spec verdicts.
"""


THIN_OUT_OF_SCOPE_BODY = """## Description

A spec whose Out of scope section is tautological.

## Functional Specification

### Inputs

A bundle.

### Outputs

A verdict.

### State

Stateless.

### Behaviour

Judge the spec.

### Invariants

Bundle hash echoed.

### Error handling

Exit 0 on success.

### Boundaries

In scope: judging the spec body.

## Out of scope

- Not in scope: anything else.
"""


MISSING_FUNCTIONAL_SPEC_BODY = """## Description

A spec whose Functional Specification section is entirely missing.

## Out of scope

- Persistence.
"""


def build_bundle(
    *,
    bundle_hash: str = "specbund1",
    spec_iri: str = "urn:spec-proposal:REQ-007",
    request_id: str = "REQ-007",
    request_body: str = "Author a worker that judges feature_spec drafts against their request.",
    spec_body: str = CONFORMING_BODY,
    spec_kind: str = "new",
    gap: GapSpecRecord | None = None,
) -> SpecQualityInput:
    """Construct a SpecQualityInput for tests."""
    if spec_kind == "new":
        proposal = SpecProposalRecord(
            iri=spec_iri,
            kind="new",
            bundle_hash=bundle_hash,
            new=NewSpecRecord(
                title="Spec-quality worker package",
                body=spec_body,
                proposed_depends_on=["FT-129", "FT-127"],
                proposed_adrs=["ADR-073", "ADR-074", "ADR-047"],
                proposed_domains=[],
                rationale=(
                    "Mirrors the tc-quality pattern. Reads SpecProposal + request, "
                    "calls Claude with the rubric prompt, emits QualityVerdict."
                ),
            ),
        )
    else:
        proposal = SpecProposalRecord(
            iri=spec_iri,
            kind="gap",
            bundle_hash=bundle_hash,
            gap=gap
            or GapSpecRecord(
                missing_information=["scope", "boundary"],
                reason="Brief is too terse.",
            ),
        )

    return SpecQualityInput(
        spec_proposal=proposal,
        request=RequestRecord(
            id=request_id,
            title="Spec-quality worker",
            body=request_body,
            source="test fixture",
        ),
        body_schema=BodySchemaRecord(
            required_h2_sections=["Description", "Functional Specification", "Out of scope"],
            required_h3_subsections=[
                "Inputs",
                "Outputs",
                "State",
                "Behaviour",
                "Invariants",
                "Error handling",
                "Boundaries",
            ],
        ),
        related_features=[
            FeatureRecord(
                id="FT-127",
                title="tc-quality worker",
                description="Judges TcProposal artifacts.",
                boundaries="In scope: worker package; out of scope: persistence.",
            ),
        ],
        central_adrs=[
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
        domain_registry=[
            DomainRecord(name="api", description="CLI / MCP / worker contracts."),
            DomainRecord(name="observability", description="Tracing, telemetry."),
        ],
        rubric=SpecQualityRubricRecord(
            criteria=[
                "schema-conforming",
                "request-faithful",
                "bounded",
                "non-colliding",
                "linkage-sound",
            ],
            description="Five-criterion rubric for SpecProposal quality.",
        ),
        authority=AuthorityRecord(
            may_decide=["phrasing", "section-order-within-h2"],
            must_escalate=["request-scope", "central-adr-changes"],
            rationale="spec-quality decides phrasing, escalates structural request changes.",
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
