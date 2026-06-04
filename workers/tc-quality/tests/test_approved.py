"""TC-290: tc-quality emits approved verdict for proposals clearing every rubric criterion."""

from tc_quality.bundle import (
    AuthorityRecord,
    ProposedNewRecord,
    ProposedTcRecord,
    TcQualityInput,
    TcQualityRubricRecord,
    TcProposalRecord,
)
from tc_quality.worker import run_judge


def _mock_caller_approved(system: str, user: str, model_id: str, max_tokens: int):
    """Mock caller that returns an approved verdict."""
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
        "verdict": "approved",
        "rationale": "All proposed TCs clear the five rubric criteria: clear, testable, non-redundant, faithful to spec, and runner-wireable.",
        "judges": "urn:tc-proposal:FT-127",
        "against": ["urn:feature:FT-127"],
        "violates": [],
        "bundle_hash": bundle_hash,
    }
    return json.dumps(verdict), 100, 50


def test_approved_verdict():
    """TC-290: approved verdict for proposals clearing every rubric criterion."""
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
            bundle_hash="abc12345",
            new=ProposedNewRecord(
                tcs=[
                    ProposedTcRecord(
                        id="TC-001",
                        title="Worker emits approved verdict",
                        body="When all criteria pass, emit approved.",
                        runner="pytest",
                        runner_args="tests/test_approved.py",
                        runner_timeout="60s",
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
            may_decide=["style", "naming"],
            must_escalate=["spec-changes"],
            rationale="tc-quality decides style, escalates spec issues.",
        ),
        bundle_hash="abc12345",
        model_id="claude-sonnet-4.5",
        endpoint="anthropic",
        parameters={},
        max_tokens=4096,
    )

    result = run_judge(bundle, caller=_mock_caller_approved)

    assert result.verdict.verdict == "approved"
    assert len(result.verdict.rationale) >= 20
    assert result.verdict.violates == []
    assert result.verdict.amendment_guidance is None
    assert result.verdict.judges
    assert result.verdict.against
    assert result.verdict.bundle_hash == "abc12345"
