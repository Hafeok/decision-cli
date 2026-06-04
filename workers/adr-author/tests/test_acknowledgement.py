"""TC-303 — adr-author returns acknowledgement with reasoning >= 40 chars.

Acceptance criteria (from FT-130 / TC-303):

- Parsed stdout deserialises to an AdrProposal whose kind equals
  "acknowledgement".
- acknowledgement.reasoning is at least 40 chars.
- acknowledgement.acknowledges references an existing ADR id in the
  bundle (here: ADR-014).
- The proposal echoes the input bundle_hash.
- The worker exits with status code 0.
"""

from __future__ import annotations

import io
import sys

from _helpers import (
    BUNDLE_HASH,
    assert_no_anthropic_attempt,
    build_bundle_for_acknowledgement,
    make_caller,
)


SUBSTANTIVE_REASONING = (
    "ADR-014 already establishes that code-quality rules live as "
    "cross-cutting ADRs with linked TCs and shell-script enforcement; "
    "FT-902 is precisely an instance of that pattern and should link to "
    "ADR-014 rather than re-derive its framing."
)


def _approved_ack_response(bundle_hash: str) -> dict:
    return {
        "kind": "acknowledgement",
        "bundle_hash": bundle_hash,
        "acknowledgement": {
            "acknowledges": "ADR-014",
            "target_feature": "FT-902",
            "reasoning": SUBSTANTIVE_REASONING,
            "rationale": (
                "Linking ADR-014 closes the preflight gap without "
                "introducing a duplicate cross-cutting decision."
            ),
        },
    }


def test_tc_303_run_author_returns_acknowledgement(monkeypatch) -> None:
    """Worker-API path: existing ADR governs the feature; acknowledgement is emitted."""
    from adr_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_acknowledgement()
    caller = make_caller(_approved_ack_response(bundle.bundle_hash))

    result = run_author(bundle, caller=caller)

    assert result.proposal.kind == "acknowledgement"
    assert result.proposal.acknowledgement is not None
    assert result.proposal.new is None
    assert result.proposal.gap is None
    assert result.proposal.bundle_hash == bundle.bundle_hash == BUNDLE_HASH

    ack = result.proposal.acknowledgement
    assert len(ack.reasoning) >= 40, (
        f"reasoning must be ≥ 40 chars per FT-130 §4B; got {len(ack.reasoning)}"
    )
    assert ack.acknowledges == "ADR-014"
    assert ack.acknowledges in bundle.central_adr_ids
    assert result.telemetry.attempts == 1
    assert len(caller.calls) == 1


def test_tc_303_cli_round_trip(monkeypatch, tmp_path) -> None:
    """CLI path: acknowledgement serialises correctly and the worker exits 0."""
    from adr_author import __main__ as cli
    from adr_author import worker as worker_mod
    from adr_author.output import AdrProposal

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_acknowledgement()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(_approved_ack_response(bundle.bundle_hash))

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

    assert proposal.kind == "acknowledgement"
    assert proposal.acknowledgement is not None
    assert proposal.new is None
    assert proposal.gap is None
    assert proposal.bundle_hash == bundle.bundle_hash
    assert len(proposal.acknowledgement.reasoning) >= 40
    assert proposal.acknowledgement.acknowledges == "ADR-014"
