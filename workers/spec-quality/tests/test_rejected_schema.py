"""TC-317: spec-quality emits rejected with violates for proposals missing required H2/H3 sections."""

from __future__ import annotations

import json

from spec_quality.worker import run_judge

from _helpers import MISSING_FUNCTIONAL_SPEC_BODY, build_bundle, extract_bundle_hash


def _mock_caller_rejected(system: str, user: str, model_id: str, max_tokens: int):
    """Mock caller that returns a rejection naming the missing Functional Specification section."""
    bundle_hash = extract_bundle_hash(user)

    verdict = {
        "verdict": "rejected",
        "rationale": (
            "The proposal body is not schema-conforming: the required H2 "
            "`Functional Specification` is missing entirely, taking with it every "
            "required H3 subsection. The schema-conforming rubric criterion fails."
        ),
        "judges": "urn:spec-proposal:REQ-007",
        "against": ["urn:request:REQ-007"],
        "violates": [
            "Functional Specification",
            "Functional Specification > Inputs",
            "Functional Specification > Outputs",
            "Functional Specification > Invariants",
        ],
        "bundle_hash": bundle_hash,
    }
    return json.dumps(verdict), 100, 50


def test_rejected_verdict_for_missing_required_sections():
    """TC-317: rejected verdict enumerates the missing H2/H3 section markers in `violates`."""
    bundle = build_bundle(
        bundle_hash="rejct12a",
        spec_iri="urn:spec-proposal:REQ-007",
        spec_body=MISSING_FUNCTIONAL_SPEC_BODY,
    )

    result = run_judge(bundle, caller=_mock_caller_rejected)

    # Worker exits with code 0 (rejection is a successful judgement, not a worker error):
    # asserted implicitly — run_judge returned, and pytest exit-code reflects this test passing.
    assert result.verdict.verdict == "rejected"

    # Stdout carries the verdict JSON: rationale references the schema-conforming criterion.
    assert "schema-conforming" in result.verdict.rationale.lower()
    assert len(result.verdict.rationale) >= 20

    # violates is a non-empty list naming the missing section markers via stable identifiers.
    assert result.verdict.violates
    assert any("Functional Specification" in v for v in result.verdict.violates)
    assert any(">" in v for v in result.verdict.violates)  # H3 markers via `>` separator

    # judges resolves to the SpecProposal IRI; bundle_hash echoes the input.
    assert result.verdict.judges == "urn:spec-proposal:REQ-007"
    assert result.verdict.bundle_hash == "rejct12a"

    # rejected verdicts carry no amendment_guidance.
    assert result.verdict.amendment_guidance is None
