"""TC-302 — adr-author returns new ADR for a gap warranting a net-new decision.

Acceptance criteria (from FT-130 / TC-302):

- Parsed stdout deserialises to an AdrProposal whose kind equals "new".
- new.body contains all four required H2 headers (Context, Decision,
  Rejected alternatives, Consequences).
- new.scope is in the controlled enum (cross-cutting, platform, domain,
  feature-specific).
- The proposal echoes the input bundle_hash.
- The worker exits with status code 0.
"""

from __future__ import annotations

import io
import sys

from _helpers import (
    BUNDLE_HASH,
    CONFORMING_ADR_BODY,
    assert_no_anthropic_attempt,
    build_bundle_for_new,
    make_caller,
)


def _approved_new_response(bundle_hash: str, gap_payload: dict) -> dict:
    return {
        "kind": "new",
        "bundle_hash": bundle_hash,
        "new": {
            "title": "Deterministic planner dispatch ordering",
            "body": CONFORMING_ADR_BODY,
            "scope": "cross-cutting",
            "proposed_domains": ["observability"],
            "addresses_gap": gap_payload,
            "rationale": (
                "Closes the preflight gap by establishing a deterministic "
                "tie-break rule and citing the trade-offs explicitly."
            ),
        },
    }


def test_tc_302_run_author_returns_new_adr(monkeypatch) -> None:
    """Worker-API path: inject a mocked caller and verify the proposal shape."""
    from adr_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_new()
    gap_payload = bundle.preflight_gap.model_dump()
    caller = make_caller(_approved_new_response(bundle.bundle_hash, gap_payload))

    result = run_author(bundle, caller=caller)

    assert result.proposal.kind == "new"
    assert result.proposal.new is not None
    assert result.proposal.acknowledgement is None
    assert result.proposal.gap is None
    assert result.proposal.bundle_hash == bundle.bundle_hash == BUNDLE_HASH
    assert result.proposal.new.scope in (
        "cross-cutting",
        "platform",
        "domain",
        "feature-specific",
    )
    assert result.telemetry.attempts == 1
    assert len(caller.calls) == 1

    body = result.proposal.new.body
    for h2 in (
        "## Context",
        "## Decision",
        "## Rejected alternatives",
        "## Consequences",
    ):
        assert h2 in body, f"expected H2 `{h2}` in body"


def test_tc_302_cli_round_trip(monkeypatch, tmp_path) -> None:
    """CLI path: write the bundle to a file, drive __main__, parse stdout."""
    from adr_author import __main__ as cli
    from adr_author import worker as worker_mod
    from adr_author.output import AdrProposal

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_new()
    gap_payload = bundle.preflight_gap.model_dump()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(_approved_new_response(bundle.bundle_hash, gap_payload))

    monkeypatch.setattr(worker_mod, "_stub_caller", caller)
    monkeypatch.setenv("ADR_AUTHOR_STUB", "1")

    stdout = io.StringIO()
    stderr = io.StringIO()
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(sys, "stderr", stderr)

    code = cli.main(["--bundle", str(bundle_path)])

    assert code == 0, f"expected exit 0; stderr={stderr.getvalue()}"

    raw = stdout.getvalue().strip()
    assert raw, "stdout must contain an AdrProposal JSON object"

    proposal = AdrProposal.model_validate_json(raw)
    assert proposal.kind == "new"
    assert proposal.new is not None
    assert proposal.acknowledgement is None
    assert proposal.gap is None
    assert proposal.bundle_hash == bundle.bundle_hash
    assert proposal.new.scope in (
        "cross-cutting",
        "platform",
        "domain",
        "feature-specific",
    )

    body = proposal.new.body
    for h2 in (
        "## Context",
        "## Decision",
        "## Rejected alternatives",
        "## Consequences",
    ):
        assert h2 in body, f"expected H2 `{h2}` in CLI-emitted body"
