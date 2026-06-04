"""TC-293: tc-quality verdict carries dec:judges and dec:against per ADR-074 polymorphism."""

from tc_quality.bundle import (
    AuthorityRecord,
    ProposedNewRecord,
    ProposedTcRecord,
    TcQualityInput,
    TcQualityRubricRecord,
    TcProposalRecord,
)
from tc_quality.worker import run_judge


def _mock_caller_judges_against(system: str, user: str, model_id: str, max_tokens: int):
    """Mock caller that returns a verdict with judges and against fields."""
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
        "rationale": "All proposed TCs clear the five rubric criteria with well-formed judges and against fields.",
        "judges": "urn:tc-proposal:FT-127",
        "against": ["urn:feature:FT-127"],
        "violates": [],
        "bundle_hash": bundle_hash,
    }
    return json.dumps(verdict), 100, 50


def test_judges_against_provenance():
    """TC-293: verdict carries dec:judges and dec:against per ADR-074 polymorphism."""
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
            bundle_hash="jkl23456",
            new=ProposedNewRecord(
                tcs=[
                    ProposedTcRecord(
                        id="TC-004",
                        title="Verdict carries judges and against",
                        body="QualityVerdict has judges (TcProposal IRI) and against (feature_spec IRI).",
                        runner="pytest",
                        runner_args="tests/test_judges.py",
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
        bundle_hash="jkl23456",
        model_id="claude-sonnet-4.5",
        endpoint="anthropic",
        parameters={},
        max_tokens=4096,
    )

    result = run_judge(bundle, caller=_mock_caller_judges_against)

    # Assert judges is populated and resolves to a TcProposal-class IRI
    assert result.verdict.judges
    assert "tc-proposal" in result.verdict.judges.lower() or "FT-127" in result.verdict.judges

    # Assert against is a non-empty list containing the feature_spec IRI
    assert result.verdict.against
    assert len(result.verdict.against) >= 1
    assert any("FT-127" in iri or "feature" in iri.lower() for iri in result.verdict.against)

    # Assert the IRIs are well-formed strings
    assert isinstance(result.verdict.judges, str)
    assert all(isinstance(iri, str) for iri in result.verdict.against)

    # Assert bundle_hash is echoed
    assert result.verdict.bundle_hash == "jkl23456"
