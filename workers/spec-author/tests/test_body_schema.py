"""TC-300 — spec-author proposal body passes FT-055/ADR-047 body-completeness validation.

Acceptance criteria (from FT-129 / TC-300):

- The FT-055 body-completeness validator returns zero W030 warnings on
  the proposal body.
- The validator's errors collection is empty.
- The validator's section coverage report shows all required H2/H3 markers.
- The worker exits with status code 0.
"""

from __future__ import annotations

from _helpers import (
    BUNDLE_HASH,
    CONFORMING_BODY,
    REQUIRED_H2,
    REQUIRED_H3,
    assert_no_anthropic_attempt,
    build_bundle_for_new,
    make_caller,
)


def _approved_new_response(bundle_hash: str) -> dict:
    return {
        "kind": "new",
        "bundle_hash": bundle_hash,
        "new": {
            "title": "Hypothetical worker package",
            "body": CONFORMING_BODY,
            "proposed_depends_on": ["FT-126"],
            "proposed_adrs": ["ADR-008", "ADR-073"],
            "proposed_domains": ["api"],
            "rationale": (
                "Addresses REQ-001 by mirroring FT-126's worker shape "
                "and covering every H2/H3 section."
            ),
        },
    }


def test_tc_300_body_passes_completeness_validator(monkeypatch) -> None:
    """Run the proposal body through the body-completeness validator."""
    from spec_author.body_check import check_body_completeness
    from spec_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_new()
    caller = make_caller(_approved_new_response(bundle.bundle_hash))

    result = run_author(bundle, caller=caller)

    assert result.proposal.kind == "new"
    assert result.proposal.new is not None

    check = check_body_completeness(
        result.proposal.new.body,
        bundle.body_schema.required_h2_sections,
        bundle.body_schema.required_h3_subsections,
    )

    # FT-055 / W030: zero warnings, zero errors.
    assert check.warnings == [], f"expected zero W030 warnings; got {check.warnings}"
    assert check.errors == [], f"expected zero errors; got {check.errors}"
    assert check.passes is True

    # Section coverage covers every required marker.
    for h2 in REQUIRED_H2:
        assert h2 in check.h2_present, f"missing H2 `{h2}` in coverage report"
    for h3 in REQUIRED_H3:
        assert h3 in check.h3_present, f"missing H3 `{h3}` in coverage report"


def test_tc_300_validator_flags_missing_sections() -> None:
    """Sanity check the validator: a stripped body raises the expected warnings."""
    from spec_author.body_check import check_body_completeness

    stripped = "## Description\n\nOnly description.\n"
    check = check_body_completeness(stripped, REQUIRED_H2, REQUIRED_H3)
    assert check.passes is False
    assert any("Functional Specification" in w for w in check.warnings)
    assert any("Out of scope" in w for w in check.warnings)
    # H3s also missing.
    for h3 in REQUIRED_H3:
        assert any(h3 in w for w in check.warnings)


def test_tc_300_worker_retries_then_falls_back_to_gap(monkeypatch) -> None:
    """If the model emits a body missing required sections twice, the
    worker falls back to a gap proposal so the harness never receives
    a malformed body."""
    from spec_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_new()

    bad_body = "## Description\n\nOnly description, missing the other sections.\n"
    bad_response = {
        "kind": "new",
        "bundle_hash": bundle.bundle_hash,
        "new": {
            "title": "Will fail",
            "body": bad_body,
            "proposed_depends_on": [],
            "proposed_adrs": [],
            "proposed_domains": [],
            "rationale": "Intentional bad body to exercise the retry fallback.",
        },
    }
    caller = make_caller(bad_response, bad_response)

    result = run_author(bundle, caller=caller)

    assert result.proposal.kind == "gap"
    assert result.proposal.gap is not None
    assert result.proposal.new is None
    assert result.proposal.bundle_hash == BUNDLE_HASH
    assert result.telemetry.attempts == 2
    assert len(caller.calls) == 2
