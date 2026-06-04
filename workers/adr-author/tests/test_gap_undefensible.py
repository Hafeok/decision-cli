"""TC-305 — adr-author returns gap when neither a new ADR nor a reasoned ack is defensible.

Acceptance criteria (from FT-130 / TC-305):

- Parsed stdout deserialises to an AdrProposal whose kind equals "gap".
- gap.missing_information has length >= 1.
- gap.reason is a non-empty string.
- Neither the new payload nor the acknowledgement payload is populated.
- The worker exits with status code 0.
"""

from __future__ import annotations

import io
import sys

from _helpers import (
    assert_no_anthropic_attempt,
    build_bundle_for_gap_undefensible,
    make_caller,
)


def _gap_response(bundle_hash: str) -> dict:
    return {
        "kind": "gap",
        "bundle_hash": bundle_hash,
        "gap": {
            "missing_information": [
                "scope",
                "problem statement",
                "candidate ADRs",
            ],
            "reason": (
                "Bundle does not name a concrete decision to be made and "
                "carries no existing ADR candidates that plausibly govern "
                "the feature. Authoring would require fabricating both the "
                "decision space and its alternatives."
            ),
        },
    }


def test_tc_305_run_author_returns_gap(monkeypatch) -> None:
    """Worker-API path: under-specified brief yields kind='gap'."""
    from adr_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_gap_undefensible()
    caller = make_caller(_gap_response(bundle.bundle_hash))

    result = run_author(bundle, caller=caller)

    assert result.proposal.kind == "gap"
    assert result.proposal.gap is not None
    assert result.proposal.new is None
    assert result.proposal.acknowledgement is None
    assert len(result.proposal.gap.missing_information) >= 1
    assert result.proposal.gap.reason.strip() != ""
    assert result.telemetry.attempts == 1
    assert len(caller.calls) == 1


def test_tc_305_cli_round_trip(monkeypatch, tmp_path) -> None:
    """CLI path: gap is serialised correctly and the worker exits 0."""
    from adr_author import __main__ as cli
    from adr_author import worker as worker_mod
    from adr_author.output import AdrProposal

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_gap_undefensible()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(_gap_response(bundle.bundle_hash))

    monkeypatch.setattr(worker_mod, "_stub_caller", caller)
    monkeypatch.setenv("ADR_AUTHOR_STUB", "1")

    stdout = io.StringIO()
    stderr = io.StringIO()
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(sys, "stderr", stderr)

    code = cli.main(["--bundle", str(bundle_path)])

    assert code == 0, f"expected exit 0; stderr={stderr.getvalue()}"

    raw = stdout.getvalue().strip()
    proposal = AdrProposal.model_validate_json(raw)

    assert proposal.kind == "gap"
    assert proposal.gap is not None
    assert proposal.new is None
    assert proposal.acknowledgement is None
    assert len(proposal.gap.missing_information) >= 1
    assert proposal.gap.reason.strip() != ""


def test_tc_305_gap_serialisation_excludes_new_and_ack() -> None:
    """The serialised JSON for a gap proposal MUST NOT carry `new` or `acknowledgement` keys."""
    import json

    from adr_author.output import AdrProposal, GapProposal

    proposal = AdrProposal(
        kind="gap",
        bundle_hash="deadbeef" * 8,
        gap=GapProposal(
            missing_information=["scope"],
            reason="brief is silent on scope",
        ),
    )
    payload = json.loads(proposal.model_dump_json(exclude_none=True))
    assert "new" not in payload
    assert "acknowledgement" not in payload
    assert payload["kind"] == "gap"
    assert payload["gap"]["missing_information"] == ["scope"]
