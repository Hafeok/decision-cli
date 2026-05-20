"""Model invariants for the verifier's VerifierInput / VerificationVerdict surface.

These tests harden the ADR-018 SHACL shape on the Python side: any
malformed verdict the worker would produce is refused by Pydantic before
the harness writes it. They cover TC-029 (SHACL conformance) and TC-030
(rejected verdict cites at least one TC/ADR) on the worker side.
"""

from __future__ import annotations

import pytest
from pydantic import ValidationError

from verifier.bundle import AdrRef, TcRef, VerifierInput
from verifier.output import MIN_RATIONALE_LENGTH, VerificationVerdict


# ---------------------------------------------------------------------------
# VerifierInput parsing
# ---------------------------------------------------------------------------


def _minimal_input(**overrides) -> dict:
    base = {
        "dispatch_id": "urn:dec:dispatch:1",
        "dispatch_group": "urn:dec:group:1",
        "interpretation_session": "urn:dec:session:interp-1",
        "action_session": "urn:dec:session:action-1",
        "feature_id": "FT-013",
        "feature_spec": "# FT-013 — implementer worker\n\nspec body",
        "produced_artifact": "diff: file foo.rs +1 -0",
        "bundle_hash": "deadbeef" * 8,
        "in_stream": "https://decision-cli.dev/stream/test",
        "relevant_tcs": [{"id": "TC-013", "type": "invariant", "body": "tc body"}],
        "relevant_adrs": [{"id": "ADR-008", "scope": "cross-cutting", "body": "adr body"}],
    }
    base.update(overrides)
    return base


def test_verifier_input_requires_core_fields() -> None:
    with pytest.raises(ValidationError):
        VerifierInput.model_validate({})


def test_verifier_input_round_trip() -> None:
    b = VerifierInput.model_validate(_minimal_input())
    assert b.feature_id == "FT-013"
    assert b.relevant_tcs[0].id == "TC-013"
    assert b.relevant_adrs[0].id == "ADR-008"
    assert b.model_id == "claude-sonnet-4-5"
    assert "TC-013" in b.references and "ADR-008" in b.references


def test_verifier_input_accepts_empty_tcs_and_adrs() -> None:
    b = VerifierInput.model_validate(
        _minimal_input(relevant_tcs=[], relevant_adrs=[])
    )
    assert b.relevant_tcs == []
    assert b.relevant_adrs == []


def test_tc_ref_defaults() -> None:
    tc = TcRef.model_validate({"id": "TC-001"})
    assert tc.type == ""
    assert tc.body == ""


def test_adr_ref_defaults() -> None:
    adr = AdrRef.model_validate({"id": "ADR-001"})
    assert adr.scope == ""
    assert adr.body == ""


# ---------------------------------------------------------------------------
# VerificationVerdict — TC-029 SHACL conformance on the Python side
# ---------------------------------------------------------------------------


def test_approved_verdict_minimal() -> None:
    v = VerificationVerdict(
        verdict="approved",
        rationale="Produced artifact satisfies all listed TCs.",
    )
    assert v.verdict == "approved"
    assert v.violates == []
    assert v.amendment_guidance is None


def test_rationale_too_short_is_rejected() -> None:
    with pytest.raises(ValidationError):
        VerificationVerdict(verdict="approved", rationale="ok")


def test_rationale_minimum_boundary() -> None:
    rationale = "x" * MIN_RATIONALE_LENGTH
    v = VerificationVerdict(verdict="approved", rationale=rationale)
    assert v.rationale == rationale


def test_unknown_verdict_value_rejected() -> None:
    with pytest.raises(ValidationError):
        VerificationVerdict(
            verdict="maybe",  # type: ignore[arg-type]
            rationale="long enough rationale body here",
        )


def test_extra_fields_refused() -> None:
    with pytest.raises(ValidationError):
        VerificationVerdict.model_validate(
            {
                "verdict": "approved",
                "rationale": "long enough rationale body here",
                "confidence": 0.9,
            }
        )


def test_approved_verdict_must_not_carry_amendment_guidance() -> None:
    with pytest.raises(ValidationError):
        VerificationVerdict(
            verdict="approved",
            rationale="long enough rationale body here",
            amendment_guidance="should not be here",
        )


# ---------------------------------------------------------------------------
# TC-030 — rejected and amendment-required verdicts must cite ≥ 1 TC/ADR
# ---------------------------------------------------------------------------


def test_rejected_verdict_without_citations_is_rejected() -> None:
    with pytest.raises(ValidationError):
        VerificationVerdict(
            verdict="rejected",
            rationale="The produced artifact missed the required TC entirely.",
            violates=[],
        )


def test_rejected_verdict_with_citation_is_accepted() -> None:
    v = VerificationVerdict(
        verdict="rejected",
        rationale="The produced artifact missed TC-013 entirely.",
        violates=["TC-013"],
    )
    assert v.violates == ["TC-013"]


def test_amendment_required_without_citations_is_rejected() -> None:
    with pytest.raises(ValidationError):
        VerificationVerdict(
            verdict="amendment-required",
            rationale="On the right track but missing the ADR-005 inStream tag.",
            violates=[],
            amendment_guidance="Add dec:inStream to the verdict quads.",
        )


def test_amendment_required_without_guidance_is_rejected() -> None:
    with pytest.raises(ValidationError):
        VerificationVerdict(
            verdict="amendment-required",
            rationale="On the right track but missing the ADR-005 inStream tag.",
            violates=["ADR-005"],
        )


def test_amendment_required_with_blank_guidance_is_rejected() -> None:
    with pytest.raises(ValidationError):
        VerificationVerdict(
            verdict="amendment-required",
            rationale="On the right track but missing the ADR-005 inStream tag.",
            violates=["ADR-005"],
            amendment_guidance="   ",
        )


def test_amendment_required_full_shape() -> None:
    v = VerificationVerdict(
        verdict="amendment-required",
        rationale="On the right track but missing the ADR-005 inStream tag.",
        violates=["ADR-005"],
        amendment_guidance="Add the dec:inStream triple to every persisted verdict.",
    )
    assert v.verdict == "amendment-required"
    assert v.amendment_guidance is not None
