"""System and user prompt templates for tc-quality (FT-127)."""

from __future__ import annotations

from .bundle import TcQualityInput

SYSTEM_PROMPT = """You are the tc-quality judge role in the decision-cli orchestration system.

Your job: judge a TcProposal (the output of the tc-author role) for fitness as an implementer bundle. A TC is fit when it is clear, testable, non-redundant, faithful to the spec, and has a wireable runner.

Score each proposed TC against the five-criterion rubric provided in the bundle. If the proposal passes (every TC clears all five criteria for kind='new'/'augment', or the coverage_map holds for kind='sufficient'), emit verdict='approved'. If the proposal violates the spec or the schema, emit verdict='rejected' with cited violates. If the proposal has fixable mayDecide-class issues (style, naming, runner-timeout numeric shape), emit verdict='amendment-required' with amendment_guidance.

Refer to the authority declaration in the bundle for mayDecide vs. mustEscalate scope. Your verdict carries one of three outcomes:

- **approved**: the proposal is fit for downstream use. The planner will auto-accept (ADR-075) and the feature's readiness bit flips.
- **rejected**: the proposal violates the spec, the schema, or a critical rubric criterion. The planner will re-dispatch tc-author with the rejection as context.
- **amendment-required**: the proposal has fixable issues within mayDecide scope. The planner will re-dispatch tc-author with amendment_guidance as additional bundle context.

Constraints:

- rationale must be at least 20 characters (ADR-018).
- rejected and amendment-required verdicts MUST cite at least one violated reference via 'violates'.
- amendment-required verdicts MUST carry 'amendment_guidance' with concrete instructions.
- Echo the bundle_hash verbatim in your verdict.
- judges is the IRI of the TcProposal under judgment; against is the list of source-of-truth IRIs (the feature_spec the TC must serve).

The rubric (five criteria):

1. **Clear**: title and body unambiguous; the test's claim is stated declaratively.
2. **Testable**: a reader can construct a pass/fail oracle from the body (per ADR-013 two-tier exit-code contract).
3. **Non-redundant**: distinct from every other proposed TC AND from every existing TC (semantic redundancy, not literal text).
4. **Faithful to spec**: the behaviour asserted is in the feature_spec (not a hallucinated requirement) and the spec asserts it (not just alludes to it).
5. **Runner-wireable**: runner is in the controlled vocabulary; runner_args is non-empty and matches the runner kind's args_pattern; runner_timeout is well-formed.

For kind='sufficient' proposals, instead validate the coverage_map against the feature_spec and existing TCs: does the claim hold that existing TCs cover the axes declared?

Emit a QualityVerdict as JSON. Your response will be parsed by Pydantic strict validation; malformed output fails the dispatch.
"""


def build_user_prompt(bundle: TcQualityInput) -> str:
    """Render the tc-quality user prompt from the bundle (FT-127)."""
    lines = [
        "# TC Quality Judgment",
        "",
        f"**Feature ID**: {bundle.feature_id}",
        "",
        "## Feature Spec",
        "",
        bundle.feature_spec.strip(),
        "",
        "## TC Proposal (artifact under judgment)",
        "",
        f"**Proposal kind**: {bundle.tc_proposal.kind}",
        f"**Bundle hash (echo this verbatim in your verdict)**: {bundle.bundle_hash}",
        "",
    ]

    if bundle.tc_proposal.kind == "new" and bundle.tc_proposal.new is not None:
        lines.append("### Proposed TCs (new)")
        lines.append("")
        for i, tc in enumerate(bundle.tc_proposal.new.tcs, start=1):
            lines.append(f"#### Proposed TC {i}: {tc.title}")
            lines.append("")
            lines.append(f"**ID**: {tc.id}")
            lines.append(f"**Runner**: {tc.runner}")
            lines.append(f"**Runner args**: {tc.runner_args}")
            lines.append(f"**Runner timeout**: {tc.runner_timeout}")
            lines.append(f"**Observes**: {', '.join(tc.observes) if tc.observes else '(none)'}")
            lines.append("")
            lines.append("**Body**:")
            lines.append("")
            lines.append(tc.body.strip())
            lines.append("")
    elif bundle.tc_proposal.kind == "augment" and bundle.tc_proposal.augment is not None:
        lines.append("### Proposed TCs (augment — additions to existing set)")
        lines.append("")
        for i, tc in enumerate(bundle.tc_proposal.augment.additions, start=1):
            lines.append(f"#### Addition {i}: {tc.title}")
            lines.append("")
            lines.append(f"**ID**: {tc.id}")
            lines.append(f"**Runner**: {tc.runner}")
            lines.append(f"**Runner args**: {tc.runner_args}")
            lines.append(f"**Runner timeout**: {tc.runner_timeout}")
            lines.append(f"**Observes**: {', '.join(tc.observes) if tc.observes else '(none)'}")
            lines.append("")
            lines.append("**Body**:")
            lines.append("")
            lines.append(tc.body.strip())
            lines.append("")
    elif bundle.tc_proposal.kind == "sufficient" and bundle.tc_proposal.sufficient is not None:
        lines.append("### Sufficient (no new TCs needed)")
        lines.append("")
        lines.append(f"**Reasoning**: {bundle.tc_proposal.sufficient.reasoning}")
        lines.append("")
        if bundle.tc_proposal.sufficient.coverage_map:
            lines.append("**Coverage map**:")
            lines.append("")
            for axis, tc_ids in bundle.tc_proposal.sufficient.coverage_map.items():
                lines.append(f"- {axis}: {', '.join(tc_ids)}")
            lines.append("")

    lines.append("## Existing TCs (for non-redundancy check)")
    lines.append("")
    if bundle.existing_tcs:
        for tc in bundle.existing_tcs:
            lines.append(f"### {tc.id}: {tc.title}")
            lines.append("")
            lines.append(f"**Status**: {tc.status}")
            lines.append(f"**Runner**: {tc.runner}")
            lines.append(f"**Runner args**: {tc.runner_args}")
            lines.append("")
            lines.append("**Body**:")
            lines.append("")
            lines.append(tc.body.strip())
            lines.append("")
    else:
        lines.append("(No existing TCs linked to this feature.)")
        lines.append("")

    lines.append("## Rubric")
    lines.append("")
    lines.append(bundle.rubric.description.strip() if bundle.rubric.description else "")
    lines.append("")
    lines.append("**Criteria**:")
    lines.append("")
    for criterion in bundle.rubric.criteria:
        lines.append(f"- {criterion}")
    lines.append("")

    lines.append("## Authority")
    lines.append("")
    lines.append(f"**May decide**: {', '.join(bundle.authority.may_decide)}")
    lines.append(f"**Must escalate**: {', '.join(bundle.authority.must_escalate)}")
    lines.append("")
    lines.append(bundle.authority.rationale.strip() if bundle.authority.rationale else "")
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
        "- **judges**: IRI of the TcProposal under judgment (use 'urn:tc-proposal:{feature_id}')"
    )
    lines.append("- **against**: list of source-of-truth IRIs (the feature_spec IRI)")
    lines.append(
        "- **violates**: list of violated references (required for rejected/amendment-required)"
    )
    lines.append(
        "- **amendment_guidance**: concrete guidance (required for amendment-required)"
    )
    lines.append(f"- **bundle_hash**: '{bundle.bundle_hash}' (echo verbatim)")
    lines.append("")

    return "\n".join(lines)


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
