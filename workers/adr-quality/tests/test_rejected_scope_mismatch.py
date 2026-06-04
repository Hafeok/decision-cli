"""TC-322: adr-quality emits rejected when a new ADR scope mismatches the gap kind or alternatives section is bare."""

from __future__ import annotations

import json

import pytest

from adr_quality.worker import run_judge

from _helpers import BARE_ALTERNATIVES_ADR_BODY, build_bundle, extract_bundle_hash


def _mock_caller_rejected_scope_mismatch(
    system: str, user: str, model_id: str, max_tokens: int
):
    """Mock caller returning a rejection citing `scope-correct`."""
    bundle_hash = extract_bundle_hash(user)

    verdict = {
        "verdict": "rejected",
        "rationale": (
            "The proposal fails the scope-correct rubric criterion: a "
            "cross-cutting preflight gap demands a cross-cutting ADR, but the "
            "proposal declares scope=feature-specific. The Decision is otherwise "
            "schema-conforming, but the wrong scope means the ADR cannot govern "
            "the cross-cutting concern."
        ),
        "judges": "urn:adr-proposal:FT-101",
        "against": [
            "urn:preflight-gap:adr:ADR-013",
            "urn:feature-spec:FT-101",
        ],
        "violates": ["scope-correct"],
        "bundle_hash": bundle_hash,
    }
    return json.dumps(verdict), 100, 50


def _mock_caller_rejected_bare_alternatives(
    system: str, user: str, model_id: str, max_tokens: int
):
    """Mock caller returning a rejection citing `alternatives-noted`."""
    bundle_hash = extract_bundle_hash(user)

    verdict = {
        "verdict": "rejected",
        "rationale": (
            "The proposal fails the alternatives-noted rubric criterion: the "
            "Rejected alternatives H2 section is present but its body is empty — "
            "no substantive alternatives with rationale are listed. The rubric "
            "requires at least two."
        ),
        "judges": "urn:adr-proposal:FT-101",
        "against": [
            "urn:preflight-gap:adr:ADR-013",
            "urn:feature-spec:FT-101",
        ],
        "violates": ["alternatives-noted"],
        "bundle_hash": bundle_hash,
    }
    return json.dumps(verdict), 100, 50


@pytest.mark.parametrize(
    ("scope_value", "body_template", "mock_caller", "expected_violation"),
    [
        (
            "feature-specific",  # scope mismatch — gap is cross-cutting
            None,  # use the conforming body
            _mock_caller_rejected_scope_mismatch,
            "scope-correct",
        ),
        (
            "cross-cutting",  # scope is fine
            BARE_ALTERNATIVES_ADR_BODY,  # bare alternatives section
            _mock_caller_rejected_bare_alternatives,
            "alternatives-noted",
        ),
    ],
    ids=["scope-mismatch", "bare-alternatives"],
)
def test_rejected_verdict_for_rubric_failure(
    scope_value, body_template, mock_caller, expected_violation
):
    """TC-322: rejected verdict naming the failing criterion (scope or alternatives)."""
    bundle_kwargs = {
        "bundle_hash": "reject12",
        "proposal_iri": "urn:adr-proposal:FT-101",
        "proposal_kind": "new",
        "proposal_scope": scope_value,
    }
    if body_template is not None:
        bundle_kwargs["proposal_body"] = body_template

    bundle = build_bundle(**bundle_kwargs)

    result = run_judge(bundle, caller=mock_caller)

    # Worker exits with code 0 (rejection is a successful judgement, not a worker error).
    assert result.verdict.verdict == "rejected"

    # violates is non-empty and names the failing criterion.
    assert result.verdict.violates
    assert expected_violation in result.verdict.violates

    # rationale names the failing criterion explicitly.
    rationale_lower = result.verdict.rationale.lower()
    assert expected_violation.replace("-", "") in rationale_lower.replace("-", "") or (
        expected_violation in rationale_lower
    )
    assert len(result.verdict.rationale) >= 20

    # judges resolves to the AdrProposal IRI; bundle_hash echoes the input.
    assert result.verdict.judges == "urn:adr-proposal:FT-101"
    assert result.verdict.bundle_hash == "reject12"

    # rejected verdicts carry no amendment_guidance.
    assert result.verdict.amendment_guidance is None
