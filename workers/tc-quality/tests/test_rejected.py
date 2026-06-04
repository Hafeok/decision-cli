"""TC-291: tc-quality emits rejected with violates citing redundant or unfaithful proposed TCs."""

from tc_quality.bundle import (
    AuthorityRecord,
    ProposedNewRecord,
    ProposedTcRecord,
    TcQualityInput,
    TcQualityRubricRecord,
    TcProposalRecord,
    TcRecord,
)
from tc_quality.worker import run_judge


def _mock_caller_rejected(system: str, user: str, model_id: str, max_tokens: int):
    """Mock caller that returns a rejected verdict."""
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
        "verdict": "rejected",
        "rationale": "Proposed TC-002 is semantically redundant with existing TC-EX1, violating the non-redundant rubric criterion.",
        "judges": "urn:tc-proposal:FT-127",
        "against": ["urn:feature:FT-127"],
        "violates": ["TC-002"],
        "bundle_hash": bundle_hash,
    }
    return json.dumps(verdict), 100, 50


def test_rejected_verdict():
    """TC-291: rejected verdict with violates citing redundant proposed TCs."""
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
            bundle_hash="def45678",
            new=ProposedNewRecord(
                tcs=[
                    ProposedTcRecord(
                        id="TC-002",
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
        existing_tcs=[
            TcRecord(
                id="TC-EX1",
                title="Worker emits approved verdict",
                body="When all criteria pass, emit approved.",
                runner="pytest",
                runner_args="tests/test_existing.py",
                runner_timeout="60s",
                status="active",
            )
        ],
        rubric=TcQualityRubricRecord(
            criteria=["clear", "testable", "non-redundant", "faithful", "runner-wireable"],
            description="Five-criterion rubric for TC quality.",
        ),
        authority=AuthorityRecord(
            may_decide=["style", "naming"],
            must_escalate=["spec-changes"],
            rationale="tc-quality decides style, escalates spec issues.",
        ),
        bundle_hash="def45678",
        model_id="claude-sonnet-4.5",
        endpoint="anthropic",
        parameters={},
        max_tokens=4096,
    )

    result = run_judge(bundle, caller=_mock_caller_rejected)

    assert result.verdict.verdict == "rejected"
    assert len(result.verdict.rationale) >= 20
    assert "non-redundant" in result.verdict.rationale
    assert result.verdict.violates
    assert "TC-002" in result.verdict.violates
    assert result.verdict.amendment_guidance is None
    assert result.verdict.bundle_hash == "def45678"
