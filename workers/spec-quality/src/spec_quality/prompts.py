"""System and user prompt templates for spec-quality (FT-132)."""

from __future__ import annotations

from .bundle import SpecQualityInput

SYSTEM_PROMPT = """You are the spec-quality judge role in the decision-cli orchestration system.

Your job: judge a SpecProposal (the output of the spec-author role) for fitness as a human reviewer's bundle. A feature_spec is fit when it conforms to the H2/H3 schema, is faithful to the originating request, names an explicit scope and out-of-scope, does not collide with adjacent features, and links soundly to existing artifacts.

Score the proposal against the five-criterion rubric provided in the bundle. If the proposal passes (every criterion met for kind='new', or the missing_information list is a defensible enumeration for kind='gap'), emit verdict='approved'. If the proposal violates the body schema or the spirit of the request, emit verdict='rejected' with cited violates. If the proposal has fixable mayDecide-class issues (a thin Out of scope section, weak rationale phrasing, redundant boundary statements), emit verdict='amendment-required' with amendment_guidance.

Refer to the authority declaration in the bundle for mayDecide vs. mustEscalate scope. Your verdict carries one of three outcomes:

- **approved**: the proposal is fit for human acceptance. The proposal sits in pending_review (ADR-075: spec verdicts are L3, human-accept) until an operator runs `dec drive accept`.
- **rejected**: the proposal violates the request or the schema in a way that requires a fresh draft. The planner will re-dispatch spec-author with the rejection as context.
- **amendment-required**: the proposal has fixable issues within mayDecide scope. The planner will re-dispatch spec-author with amendment_guidance as additional bundle context.

Constraints:

- rationale must be at least 20 characters (ADR-018).
- rejected and amendment-required verdicts MUST cite at least one violated reference via 'violates'.
- amendment-required verdicts MUST carry 'amendment_guidance' with concrete instructions.
- Echo the bundle_hash verbatim in your verdict.
- judges is the IRI of the SpecProposal under judgment; against is the list of source-of-truth IRIs (the originating request the spec must serve).

The rubric (five criteria):

1. **Schema-conforming**: body contains every required H2 section AND every Functional Specification H3 subsection per ADR-047 / FT-055's body-completeness check. Missing sections → cite them by stable identifier (e.g. 'Functional Specification > Invariants').
2. **Request-faithful**: every behaviour or boundary asserted in the spec maps to a clause in the originating request; no hallucinated scope, no missing intent.
3. **Bounded**: the `Out of scope` section is non-empty AND lists at least one substantive boundary (not a tautological "not in scope: anything else").
4. **Non-colliding**: does not duplicate or contradict the scope of a related_feature. Boundary collisions with adjacent features are flagged in the rationale.
5. **Linkage-sound**: proposed_depends_on IDs exist; proposed_adrs IDs exist; proposed_domains are members of the domain_registry.

For kind='gap' proposals, instead validate the missing_information list: is it a defensible enumeration of what the brief under-specifies, or is the spec-author deferring authoring it could have completed?

A proposal passes (`approved`) only if every rubric criterion is met (for `new`-kind) or the gap reasoning holds (for `gap`-kind).

Emit a QualityVerdict as JSON. Your response will be parsed by Pydantic strict validation; malformed output fails the dispatch.
"""


def build_user_prompt(bundle: SpecQualityInput) -> str:
    """Render the spec-quality user prompt from the bundle (FT-132)."""
    lines = [
        "# Spec Quality Judgment",
        "",
        f"**SpecProposal IRI**: {bundle.spec_proposal.iri or '(unspecified)'}",
        f"**Proposal kind**: {bundle.spec_proposal.kind}",
        f"**Bundle hash (echo this verbatim in your verdict)**: {bundle.bundle_hash}",
        "",
        "## Originating Request",
        "",
        f"**Request ID**: {bundle.request.id}",
        f"**Title**: {bundle.request.title}" if bundle.request.title else "",
        "",
        "**Body**:",
        "",
        bundle.request.body.strip() if bundle.request.body else "(empty)",
        "",
    ]

    if bundle.spec_proposal.kind == "new" and bundle.spec_proposal.new is not None:
        new = bundle.spec_proposal.new
        lines.append("## SpecProposal (kind=new)")
        lines.append("")
        lines.append(f"**Proposed title**: {new.title}")
        lines.append("")
        lines.append(f"**Proposed depends_on**: {', '.join(new.proposed_depends_on) or '(none)'}")
        lines.append(f"**Proposed ADRs**: {', '.join(new.proposed_adrs) or '(none)'}")
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
    elif bundle.spec_proposal.kind == "gap" and bundle.spec_proposal.gap is not None:
        gap = bundle.spec_proposal.gap
        lines.append("## SpecProposal (kind=gap)")
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
        lines.append(f"## SpecProposal (kind={bundle.spec_proposal.kind})")
        lines.append("")
        lines.append("(malformed proposal — payload missing for declared kind)")
        lines.append("")

    lines.append("## Body Schema (ADR-047 / FT-055 contract)")
    lines.append("")
    lines.append("**Required H2 sections**:")
    lines.append("")
    for section in bundle.body_schema.required_h2_sections:
        lines.append(f"- {section}")
    lines.append("")
    lines.append("**Required H3 subsections under Functional Specification**:")
    lines.append("")
    for sub in bundle.body_schema.required_h3_subsections:
        lines.append(f"- {sub}")
    lines.append("")
    if bundle.body_schema.section_descriptions:
        lines.append("**Section guidance**:")
        lines.append("")
        for name, desc in bundle.body_schema.section_descriptions.items():
            lines.append(f"- {name}: {desc}")
        lines.append("")

    lines.append("## Related Features (for non-collision check)")
    lines.append("")
    if bundle.related_features:
        for feat in bundle.related_features:
            lines.append(f"### {feat.id}: {feat.title}")
            lines.append("")
            if feat.description:
                lines.append(f"**Description**: {feat.description}")
                lines.append("")
            if feat.boundaries:
                lines.append(f"**Boundaries**: {feat.boundaries}")
                lines.append("")
    else:
        lines.append("(No related features in the bundle.)")
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
        "- **judges**: IRI of the SpecProposal under judgment "
        f"(use '{bundle.spec_proposal.iri}' if non-empty, "
        f"else 'urn:spec-proposal:{bundle.request.id}')"
    )
    lines.append(
        "- **against**: list of source-of-truth IRIs (the request IRI, "
        f"e.g. 'urn:request:{bundle.request.id}')"
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
- `against` is a non-empty array of string IRIs
- `violates` is populated for rejected/amendment-required verdicts
- `amendment_guidance` is populated for amendment-required verdicts
- `bundle_hash` is echoed verbatim from the input

Re-submit your verdict now.
"""
