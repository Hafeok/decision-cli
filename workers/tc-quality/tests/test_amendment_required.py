"""TC-292: tc-quality emits amendment-required with amendment_guidance for fixable issues."""

from tc_quality.bundle import (
    AuthorityRecord,
    ProposedNewRecord,
    ProposedTcRecord,
    TcQualityInput,
    TcQualityRubricRecord,
    TcProposalRecord,
)
from tc_quality.worker import run_judge


def _mock_caller_amendment(system: str, user: str, model_id: str, max_tokens: int):
    """Mock caller that returns an amendment-required verdict."""
    import json

    # Extract bundle_hash from the user prompt
    bundle_hash = ""
    for line in user.splitlines():
        if "**Bundle hash (echo this verbatim in your verdict)**:" in line:
            bundle_hash = line.split("**Bundle hash (echo this verbatim in your verdict)**:", 1)[
                1
            ].strip()
            break

    verdict = {
        "verdict": "amendment-required",
        "rationale": "TC-003 has a malformed title style (lowercase instead of sentence case) and runner_timeout is numeric instead of a time-suffix string.",
        "judges": "urn:tc-proposal:FT-127",
        "against": ["urn:feature:FT-127"],
        "violates": ["TC-003"],
        "amendment_guidance": "Change title to sentence case: 'Worker emits verdict'. Change runner_timeout from '60' to '60s'.",
        "bundle_hash": bundle_hash,
    }
    return json.dumps(verdict), 100, 50


def test_amendment_required_verdict():
    """TC-292: amendment-required verdict with amendment_guidance for fixable style issues."""
    bundle = TcQualityInput(
        feature_id="FT-127",
        feature_spec="""# FT-127 — tc-quality judge worker

## Description
Implements the tc-quality role.

## Functional Specification
Takes a TcProposal and emits a QualityVerdict.
""",
        tc_proposal=TcProposalRecord(
            kind="new",
            bundle_hash="ghi78901",
            new=ProposedNewRecord(
                tcs=[
                    ProposedTcRecord(
                        id="TC-003",
                        title="worker emits verdict",
                        body="When all criteria pass, emit approved.",
                        runner="pytest",
                        runner_args="tests/test_approved.py",
                        runner_timeout="60",
                        observes=["happy"],
                    )
                ]
            ),
        ),
        existing_tcs=[],
        rubric=TcQualityRubricRecord(
            criteria=["clear", "testable", "non-redundant", "faithful", "runner-wireable"],
            description="Five-criterion rubric for TC quality.",
        ),
        authority=AuthorityRecord(
            may_decide=["style", "naming", "timeout-format"],
            must_escalate=["spec-changes"],
            rationale="tc-quality decides style, escalates spec issues.",
        ),
        bundle_hash="ghi78901",
        model_id="claude-sonnet-4.5",
        endpoint="anthropic",
        parameters={},
        max_tokens=4096,
    )

    result = run_judge(bundle, caller=_mock_caller_amendment)

    assert result.verdict.verdict == "amendment-required"
    assert len(result.verdict.rationale) >= 20
    assert result.verdict.violates
    assert "TC-003" in result.verdict.violates
    assert result.verdict.amendment_guidance
    assert len(result.verdict.amendment_guidance) >= 20
    assert "style" in result.verdict.rationale.lower() or "timeout" in result.verdict.rationale
    assert result.verdict.bundle_hash == "ghi78901"
