"""System and user prompt templates for adr-quality (FT-133)."""

from __future__ import annotations

from .bundle import AdrQualityInput

SYSTEM_PROMPT = """You are the adr-quality judge role in the decision-cli orchestration system.

Your job: judge an AdrProposal (the output of the adr-author role) for fitness as a human reviewer's bundle. A NEW ADR is fit when it conforms to the H2 schema, soundly closes the preflight gap, has the right scope, names valid domains, and lists at least two substantive Rejected alternatives. An ACKNOWLEDGEMENT is fit when it references an existing ADR that genuinely governs the feature, with reasoning >= 40 characters that is materially relevant. A GAP-kind proposal is fit when its missing_information list is a defensible enumeration of what the brief under-specifies.

Score the proposal against the rubric below. The rubric criteria applied depend on the proposal `kind`:

For **`new` proposals** (a net-new ADR):

1. **Schema-conforming.** Body contains every required H2 section (Context, Decision, Rejected alternatives, Consequences, Status). Missing sections fail this criterion.
2. **Gap-closing.** The proposed Decision section materially addresses the preflight_gap. A gap citing ADR-XXX is closed by authoring a NEW ADR that explicitly relates to ADR-XXX in its Context section.
3. **Scope-correct.** The proposed `scope` field ('cross-cutting' / 'platform' / 'domain' / 'feature-specific') matches the gap kind: a 'cross-cutting' ADR for a cross-cutting gap, a 'feature-specific' ADR for a per-feature gap, etc. A cross-cutting gap addressed by a feature-specific ADR is a scope mismatch.
4. **Domain-valid.** Every `proposed_domains` entry is a member of the `domain_registry`.
5. **Alternatives-noted.** The Rejected alternatives section names at least two substantive alternatives with rationale. A bare or empty section fails this criterion.

For **`acknowledgement` proposals**:

1. **Reasoning length floor.** `reasoning` is at least 40 characters (FT-130 §4B no-bare-ack rule, also enforced at the action worker boundary).
2. **References existing.** `acknowledges` field is an ADR-NNN id that exists in the central ADR catalog.
3. **Reasoning relevance.** The reasoning materially explains why the referenced existing ADR governs the feature — not generic 'applies workspace-wide' boilerplate.
4. **Gap-matching.** The preflight_gap is the gap the acknowledgement addresses; the acknowledgement is not silently retargeting a different gap.
5. **Not-better-as-new.** The reasoning is not a wishful 'this existing ADR almost fits'; if the gap genuinely needs a new ADR, the verdict is `amendment-required` redirecting to a `new` proposal.

For **`gap`-kind AdrProposals**, the rubric asks: is the `missing_information` list defensible, or could adr-author have produced either a `new` or an `acknowledgement` from what it had?

A proposal passes (`approved`) only if every applicable rubric criterion is met. Bare acknowledgements (reasoning empty or < 40 chars) are categorically rejected as defense in depth.

Refer to the authority declaration in the bundle for mayDecide vs. mustEscalate scope. Your verdict carries one of three outcomes:

- **approved**: the proposal is fit for human acceptance. The proposal sits in pending_review (ADR-075: ADR verdicts are L3, human-accept) until an operator runs `dec drive accept`.
- **rejected**: the proposal violates the rubric in a way that requires a fresh draft. The planner will re-dispatch adr-author with the rejection as context.
- **amendment-required**: the proposal has fixable issues within mayDecide scope. The planner will re-dispatch adr-author with amendment_guidance as additional bundle context.

Constraints:

- rationale must be at least 20 characters (ADR-018).
- rejected and amendment-required verdicts MUST cite at least one violated reference via 'violates'.
- amendment-required verdicts MUST carry 'amendment_guidance' with concrete instructions.
- Echo the bundle_hash verbatim in your verdict.
- judges is the IRI of the AdrProposal under judgment; against is the list of source-of-truth IRIs (the preflight_gap IRI AND the feature_spec IRI).

Emit a QualityVerdict as JSON. Your response will be parsed by Pydantic strict validation; malformed output fails the dispatch.
"""


def build_user_prompt(bundle: AdrQualityInput) -> str:
    """Render the adr-quality user prompt from the bundle (FT-133)."""
    lines = [
        "# ADR Quality Judgment",
        "",
        f"**AdrProposal IRI**: {bundle.adr_proposal.iri or '(unspecified)'}",
        f"**Proposal kind**: {bundle.adr_proposal.kind}",
        f"**Feature**: {bundle.feature_id}",
        f"**Bundle hash (echo this verbatim in your verdict)**: {bundle.bundle_hash}",
        "",
        "## Feature Spec",
        "",
        bundle.feature_spec.strip() if bundle.feature_spec else "(empty)",
        "",
        "## Preflight Gap",
        "",
        f"**Kind**: {bundle.preflight_gap.kind}",
        f"**Severity**: {bundle.preflight_gap.severity}",
    ]
    if bundle.preflight_gap.adr_id:
        lines.append(f"**ADR id**: {bundle.preflight_gap.adr_id}")
    if bundle.preflight_gap.domain:
        lines.append(f"**Domain**: {bundle.preflight_gap.domain}")
    if bundle.preflight_gap.message:
        lines.append("")
        lines.append(f"**Message**: {bundle.preflight_gap.message}")
    lines.append("")

    if bundle.adr_proposal.kind == "new" and bundle.adr_proposal.new is not None:
        new = bundle.adr_proposal.new
        lines.append("## AdrProposal (kind=new)")
        lines.append("")
        lines.append(f"**Proposed title**: {new.title}")
        lines.append("")
        lines.append(f"**Proposed scope**: {new.scope}")
        lines.append(f"**Proposed domains**: {', '.join(new.proposed_domains) or '(none)'}")
        lines.append("")
        lines.append("**Body**:")
        lines.append("")
        lines.append(new.body.strip() if new.body else "(empty)")
        lines.append("")
        lines.append("**Rationale (author's)**:")
        lines.append("")
        lines.append(new.rationale.strip() if new.rationale else "(empty)")
        lines.append("")
    elif (
        bundle.adr_proposal.kind == "acknowledgement"
        and bundle.adr_proposal.acknowledgement is not None
    ):
        ack = bundle.adr_proposal.acknowledgement
        lines.append("## AdrProposal (kind=acknowledgement)")
        lines.append("")
        lines.append(f"**Acknowledges**: {ack.acknowledges}")
        lines.append(f"**Target feature**: {ack.target_feature}")
        lines.append("")
        lines.append("**Reasoning**:")
        lines.append("")
        lines.append(ack.reasoning.strip() if ack.reasoning else "(empty)")
        lines.append("")
        lines.append("**Rationale (author's)**:")
        lines.append("")
        lines.append(ack.rationale.strip() if ack.rationale else "(empty)")
        lines.append("")
    elif bundle.adr_proposal.kind == "gap" and bundle.adr_proposal.gap is not None:
        gap = bundle.adr_proposal.gap
        lines.append("## AdrProposal (kind=gap)")
        lines.append("")
        lines.append("**Missing information** (author's enumeration):")
        lines.append("")
        for item in gap.missing_information:
            lines.append(f"- {item}")
        lines.append("")
        lines.append("**Reason**:")
        lines.append("")
        lines.append(gap.reason.strip() if gap.reason else "(empty)")
        lines.append("")
    else:
        lines.append(f"## AdrProposal (kind={bundle.adr_proposal.kind})")
        lines.append("")
        lines.append("(malformed proposal — payload missing for declared kind)")
        lines.append("")

    lines.append("## ADR Body Schema")
    lines.append("")
    lines.append("**Required H2 sections** (for new-kind proposals):")
    lines.append("")
    for section in bundle.adr_body_schema.required_h2_sections:
        lines.append(f"- {section}")
    lines.append("")
    if bundle.adr_body_schema.section_descriptions:
        lines.append("**Section guidance**:")
        lines.append("")
        for name, desc in bundle.adr_body_schema.section_descriptions.items():
            lines.append(f"- {name}: {desc}")
        lines.append("")

    lines.append("## Central / Cross-cutting ADRs")
    lines.append("")
    if bundle.central_adrs:
        for adr in bundle.central_adrs:
            lines.append(f"### {adr.id}: {adr.title}")
            lines.append("")
            lines.append(f"**Scope**: {adr.scope}")
            if adr.summary:
                lines.append(f"**Summary**: {adr.summary}")
            lines.append("")
    else:
        lines.append("(No central ADRs in the bundle.)")
        lines.append("")

    lines.append("## Domain Registry")
    lines.append("")
    if bundle.domain_registry:
        for dom in bundle.domain_registry:
            if dom.description:
                lines.append(f"- {dom.name}: {dom.description}")
            else:
                lines.append(f"- {dom.name}")
    else:
        lines.append("(No domains declared.)")
    lines.append("")

    lines.append("## Rubric")
    lines.append("")
    if bundle.rubric.description:
        lines.append(bundle.rubric.description.strip())
        lines.append("")
    lines.append("**Criteria**:")
    lines.append("")
    for criterion in bundle.rubric.criteria:
        lines.append(f"- {criterion}")
    lines.append("")

    lines.append("## Authority")
    lines.append("")
    lines.append(f"**May decide**: {', '.join(bundle.authority.may_decide) or '(none)'}")
    lines.append(f"**Must escalate**: {', '.join(bundle.authority.must_escalate) or '(none)'}")
    lines.append("")
    if bundle.authority.rationale:
        lines.append(bundle.authority.rationale.strip())
        lines.append("")

    lines.append("## Your task")
    lines.append("")
    lines.append(
        "Judge the proposal against the rubric. Emit a QualityVerdict as JSON with the following fields:"
    )
    lines.append("")
    lines.append("- **verdict**: 'approved', 'rejected', or 'amendment-required'")
    lines.append("- **rationale**: substantive explanation (>= 20 chars)")
    lines.append(
        "- **judges**: IRI of the AdrProposal under judgment "
        f"(use '{bundle.adr_proposal.iri}' if non-empty, "
        f"else 'urn:adr-proposal:{bundle.feature_id}')"
    )
    lines.append(
        "- **against**: list of source-of-truth IRIs — the preflight_gap IRI "
        f"('{bundle.preflight_gap_iri}') AND the feature_spec IRI "
        f"('{bundle.resolved_feature_spec_iri}')"
    )
    lines.append(
        "- **violates**: list of violated references (required for rejected/amendment-required)"
    )
    lines.append(
        "- **amendment_guidance**: concrete guidance (required for amendment-required)"
    )
    lines.append(f"- **bundle_hash**: '{bundle.bundle_hash}' (echo verbatim)")
    lines.append("")

    return "\n".join(line for line in lines if line is not None)


def build_retry_prompt(error_message: str) -> str:
    """Construct a retry prompt after schema validation failure."""
    return f"""
# Schema Validation Failure

Your previous response failed Pydantic validation:

{error_message}

Please correct the error and emit a valid QualityVerdict JSON object. Ensure:

- `verdict` is one of: 'approved', 'rejected', 'amendment-required'
- `rationale` is at least 20 characters
- `judges` is a string IRI
- `against` is a non-empty array of string IRIs (preflight_gap IRI + feature_spec IRI)
- `violates` is populated for rejected/amendment-required verdicts
- `amendment_guidance` is populated for amendment-required verdicts
- `bundle_hash` is echoed verbatim from the input

Re-submit your verdict now.
"""
