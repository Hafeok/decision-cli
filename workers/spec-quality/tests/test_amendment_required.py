"""TC-318: spec-quality emits amendment-required with guidance for thin Out of scope sections."""

from __future__ import annotations

import json

from spec_quality.worker import run_judge

from _helpers import THIN_OUT_OF_SCOPE_BODY, build_bundle, extract_bundle_hash


def _mock_caller_amendment(system: str, user: str, model_id: str, max_tokens: int):
    """Mock caller that returns an amendment-required verdict on a thin Out of scope section."""
    bundle_hash = extract_bundle_hash(user)

    verdict = {
        "verdict": "amendment-required",
        "rationale": (
            "The body is schema-conformant — every required H2 and H3 section is "
            "present. However, the Bounded rubric criterion fails: the Out of scope "
            "section contains only a tautological restatement and lists no substantive "
            "boundary. Schema is fine; boundedness is not."
        ),
        "judges": "urn:spec-proposal:REQ-007",
        "against": ["urn:request:REQ-007"],
        "violates": ["bounded"],
        "amendment_guidance": (
            "Revise the `## Out of scope` section to name at least one substantive "
            "boundary — e.g. 'persistence is handled by the harness, not the worker' — "
            "and remove the tautological entry."
        ),
        "bundle_hash": bundle_hash,
    }
    return json.dumps(verdict), 100, 50


def test_amendment_required_for_thin_out_of_scope():
    """TC-318: amendment-required verdict with guidance for non-substantive Out of scope section."""
    bundle = build_bundle(
        bundle_hash="amend123",
        spec_iri="urn:spec-proposal:REQ-007",
        spec_body=THIN_OUT_OF_SCOPE_BODY,
    )

    result = run_judge(bundle, caller=_mock_caller_amendment)

    # Worker exits with code 0 (amendment is a successful judgement). The pytest
    # framework's own exit code reflects this assertion passing.
    assert result.verdict.verdict == "amendment-required"

    # amendment_guidance is a non-empty string >= 20 chars and names the section to revise.
    assert result.verdict.amendment_guidance is not None
    assert len(result.verdict.amendment_guidance) >= 20
    assert "out of scope" in result.verdict.amendment_guidance.lower()

    # violates is non-empty and cites the Bounded rubric criterion.
    assert result.verdict.violates
    assert any("bounded" in v.lower() for v in result.verdict.violates)

    # rationale explains schema-vs-boundedness split.
    assert len(result.verdict.rationale) >= 20
    rationale_lower = result.verdict.rationale.lower()
    assert "schema" in rationale_lower
    assert "boundedness" in rationale_lower or "bounded" in rationale_lower

    # judges + bundle_hash provenance.
    assert result.verdict.judges == "urn:spec-proposal:REQ-007"
    assert result.verdict.bundle_hash == "amend123"
