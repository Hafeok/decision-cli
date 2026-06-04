"""TC-299 — spec-author returns gap listing missing_information for an under-specified request.

Acceptance criteria (from FT-129 / TC-299):

- Parsed stdout deserialises to a SpecProposal whose kind equals "gap".
- gap.missing_information has length >= 1.
- gap.reason is a non-empty string.
- Neither `new` nor any other proposal payload is populated.
- The worker exits with status code 0.
"""

from __future__ import annotations

import io
import sys

from _helpers import (
    assert_no_anthropic_attempt,
    build_bundle_for_gap,
    make_caller,
)


def _gap_response(bundle_hash: str) -> dict:
    return {
        "kind": "gap",
        "bundle_hash": bundle_hash,
        "gap": {
            "missing_information": [
                "scope",
                "boundary",
                "constraint resolution between synchronous and 10k concurrent",
            ],
            "reason": (
                "Brief contradicts itself ('synchronous' vs '10k concurrent') and "
                "states no scope or acceptance criteria. Authoring would require "
                "inventing decisions that belong in ADRs."
            ),
        },
    }


def test_tc_299_run_author_returns_gap(monkeypatch) -> None:
    """Worker-API path: under-specified brief yields kind='gap'."""
    from spec_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_gap()
    caller = make_caller(_gap_response(bundle.bundle_hash))

    result = run_author(bundle, caller=caller)

    assert result.proposal.kind == "gap"
    assert result.proposal.gap is not None
    assert result.proposal.new is None
    assert len(result.proposal.gap.missing_information) >= 1
    assert result.proposal.gap.reason.strip() != ""
    assert result.telemetry.attempts == 1
    assert len(caller.calls) == 1


def test_tc_299_cli_round_trip(monkeypatch, tmp_path) -> None:
    """CLI path: gap is serialised correctly and the worker exits 0."""
    from spec_author import __main__ as cli
    from spec_author import worker as worker_mod
    from spec_author.output import SpecProposal

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_gap()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(_gap_response(bundle.bundle_hash))

    monkeypatch.setattr(worker_mod, "_stub_caller", caller)
    monkeypatch.setenv("SPEC_AUTHOR_STUB", "1")

    stdout = io.StringIO()
    stderr = io.StringIO()
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(sys, "stderr", stderr)

    code = cli.main(["--bundle", str(bundle_path)])

    assert code == 0, f"expected exit 0; stderr={stderr.getvalue()}"

    raw = stdout.getvalue().strip()
    proposal = SpecProposal.model_validate_json(raw)

    assert proposal.kind == "gap"
    assert proposal.gap is not None
    assert proposal.new is None
    assert len(proposal.gap.missing_information) >= 1
    assert proposal.gap.reason.strip() != ""


def test_tc_299_gap_serialisation_excludes_new() -> None:
    """The serialised JSON for a gap proposal MUST NOT carry a `new` key."""
    import json

    from spec_author.output import GapProposal, SpecProposal

    proposal = SpecProposal(
        kind="gap",
        bundle_hash="deadbeef" * 8,
        gap=GapProposal(missing_information=["scope"], reason="brief is silent on scope"),
    )
    payload = json.loads(proposal.model_dump_json(exclude_none=True))
    assert "new" not in payload
    assert payload["kind"] == "gap"
    assert payload["gap"]["missing_information"] == ["scope"]
