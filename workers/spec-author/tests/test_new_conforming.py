"""TC-298 — spec-author returns new with H2/H3-conforming body for a well-formed request.

Acceptance criteria (from FT-129 / TC-298):

- Parsed stdout deserialises to a SpecProposal whose kind field equals "new".
- The proposal's new.body contains all three required H2 headers
  (Description, Functional Specification, Out of scope).
- The body contains all seven required H3 subsections under Functional
  Specification (Inputs, Outputs, State, Behaviour, Invariants, Error handling, Boundaries).
- The proposal echoes the input bundle_hash.
- The worker exits with status code 0.
"""

from __future__ import annotations

import io
import sys

import pytest

from _helpers import (
    BUNDLE_HASH,
    CONFORMING_BODY,
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


def test_tc_298_run_author_returns_new_conforming(monkeypatch) -> None:
    """Worker-API path: inject a mocked caller and verify the proposal shape."""
    from spec_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_new()
    caller = make_caller(_approved_new_response(bundle.bundle_hash))

    result = run_author(bundle, caller=caller)

    assert result.proposal.kind == "new"
    assert result.proposal.new is not None
    assert result.proposal.gap is None
    assert result.proposal.bundle_hash == bundle.bundle_hash == BUNDLE_HASH
    assert result.telemetry.attempts == 1
    assert len(caller.calls) == 1

    body = result.proposal.new.body
    for h2 in ("## Description", "## Functional Specification", "## Out of scope"):
        assert h2 in body, f"expected H2 `{h2}` in body"
    for h3 in (
        "### Inputs",
        "### Outputs",
        "### State",
        "### Behaviour",
        "### Invariants",
        "### Error handling",
        "### Boundaries",
    ):
        assert h3 in body, f"expected H3 `{h3}` in body"


def test_tc_298_cli_round_trip(monkeypatch, tmp_path) -> None:
    """CLI path: write the bundle to a file, drive __main__, parse stdout."""
    from spec_author import __main__ as cli
    from spec_author import worker as worker_mod
    from spec_author.output import SpecProposal

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_new()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(_approved_new_response(bundle.bundle_hash))

    monkeypatch.setattr(worker_mod, "_stub_caller", caller)
    monkeypatch.setenv("SPEC_AUTHOR_STUB", "1")

    stdout = io.StringIO()
    stderr = io.StringIO()
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(sys, "stderr", stderr)

    code = cli.main(["--bundle", str(bundle_path)])

    assert code == 0, f"expected exit 0; stderr={stderr.getvalue()}"

    raw = stdout.getvalue().strip()
    assert raw, "stdout must contain a SpecProposal JSON object"

    proposal = SpecProposal.model_validate_json(raw)
    assert proposal.kind == "new"
    assert proposal.new is not None
    assert proposal.gap is None
    assert proposal.bundle_hash == bundle.bundle_hash

    body = proposal.new.body
    for h2 in ("## Description", "## Functional Specification", "## Out of scope"):
        assert h2 in body, f"expected H2 `{h2}` in CLI-emitted body"
    for h3 in (
        "### Inputs",
        "### Outputs",
        "### State",
        "### Behaviour",
        "### Invariants",
        "### Error handling",
        "### Boundaries",
    ):
        assert h3 in body, f"expected H3 `{h3}` in CLI-emitted body"


def test_tc_298_h2_match_is_case_insensitive_substring() -> None:
    """The acceptance criterion uses case-insensitive substring matching."""
    body = CONFORMING_BODY
    lower = body.lower()
    for needle in (
        "## description",
        "## functional specification",
        "## out of scope",
    ):
        assert needle in lower
