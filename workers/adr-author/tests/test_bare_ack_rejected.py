"""TC-304 — adr-author rejects bare acknowledgements at the worker boundary.

Acceptance criteria (from FT-130 / TC-304):

- When the mocked Claude returns an acknowledgement whose reasoning is
  empty (or whitespace-only), the actual stdout EITHER (a) parses to
  kind="gap" OR (b) is empty/error and exit code is non-zero.
- Under no circumstances does parsed stdout deserialise to a
  kind="acknowledgement" proposal with an empty/whitespace reasoning.
- When the exit code is non-zero, stderr contains a structured error
  message identifying the bare-ack rejection.
- Both empty-string and whitespace-only variants pass the invariants.
"""

from __future__ import annotations

import io
import json
import sys

import pytest

from _helpers import (
    assert_no_anthropic_attempt,
    build_bundle_for_bare_ack,
    make_caller,
)


BARE_REASONING_VARIANTS = ["", "   \n\t "]


def _bare_ack_response(bundle_hash: str, reasoning: str) -> dict:
    return {
        "kind": "acknowledgement",
        "bundle_hash": bundle_hash,
        "acknowledgement": {
            "acknowledges": "ADR-X",
            "target_feature": "FT-903",
            # NB: pydantic min_length=40 will reject this, but the bare-ack
            # defence is meant to fire BEFORE pydantic validation so that
            # a 40-char string of pure whitespace is also rejected. We
            # exercise both empty and whitespace-only variants below.
            "reasoning": reasoning,
            "rationale": (
                "A bare acknowledgement with no substantive reasoning "
                "would silently consume a planner cycle — must be rejected."
            ),
        },
    }


@pytest.mark.parametrize("reasoning", BARE_REASONING_VARIANTS)
def test_tc_304_run_author_rejects_bare_ack(monkeypatch, reasoning) -> None:
    """Worker-API path: bare-ack must NEVER survive past the worker boundary.

    The mocked Claude returns a bare-ack twice (initial + retry). The
    worker's retry budget exhausts and the contract requires a fallback
    to ``kind: gap`` (never an acknowledgement with empty reasoning).
    """
    from adr_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_bare_ack()
    # Both initial and retry attempts return bare-ack — worker must
    # fall back to gap rather than emit the bare acknowledgement.
    caller = make_caller(
        _bare_ack_response(bundle.bundle_hash, reasoning),
        _bare_ack_response(bundle.bundle_hash, reasoning),
    )

    result = run_author(bundle, caller=caller)

    # Invariant: under NO circumstances does the worker emit an
    # acknowledgement with empty/whitespace reasoning.
    if result.proposal.kind == "acknowledgement":
        ack = result.proposal.acknowledgement
        assert ack is not None
        assert (
            ack.reasoning.strip() != ""
        ), "bare-ack must NEVER be emitted (FT-130 §4B)"
        assert len(ack.reasoning.strip()) >= 40

    # The contract permits either (a) gap fallback or (b) non-zero exit.
    # At the worker-API level we observe the fallback to gap.
    assert result.proposal.kind == "gap"
    assert result.proposal.gap is not None
    assert len(result.proposal.gap.missing_information) >= 1
    assert result.telemetry.attempts == 2  # retry budget consumed


@pytest.mark.parametrize("reasoning", BARE_REASONING_VARIANTS)
def test_tc_304_cli_round_trip_rejects_bare_ack(
    monkeypatch, tmp_path, reasoning
) -> None:
    """CLI path: stdout MUST NOT carry a bare-ack proposal."""
    from adr_author import __main__ as cli
    from adr_author import worker as worker_mod
    from adr_author.output import AdrProposal

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_bare_ack()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(
        _bare_ack_response(bundle.bundle_hash, reasoning),
        _bare_ack_response(bundle.bundle_hash, reasoning),
    )

    monkeypatch.setattr(worker_mod, "_stub_caller", caller)
    monkeypatch.setenv("ADR_AUTHOR_STUB", "1")

    stdout = io.StringIO()
    stderr = io.StringIO()
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(sys, "stderr", stderr)

    code = cli.main(["--bundle", str(bundle_path)])

    raw = stdout.getvalue().strip()

    # Acceptance: EITHER (a) parses to kind: "gap", OR (b) empty/error
    # output with non-zero exit code.
    if code == 0:
        # Path (a): a structured proposal was emitted; it MUST be a gap,
        # not an acknowledgement with empty reasoning.
        assert raw, "exit 0 implies stdout carries a proposal JSON"
        proposal = AdrProposal.model_validate_json(raw)
        assert proposal.kind != "acknowledgement" or (
            proposal.acknowledgement is not None
            and proposal.acknowledgement.reasoning.strip() != ""
            and len(proposal.acknowledgement.reasoning.strip()) >= 40
        ), "bare-ack must NEVER appear on stdout"
        assert proposal.kind == "gap"
    else:
        # Path (b): non-zero exit; stderr identifies the bare-ack issue.
        stderr_text = stderr.getvalue()
        assert "bare-ack" in stderr_text or "acknowledgement" in stderr_text, (
            f"non-zero exit must name the bare-ack rejection; stderr={stderr_text!r}"
        )

    # Direct guard against the most dangerous failure mode: stdout
    # parsing to an acknowledgement with empty/whitespace reasoning.
    if raw:
        try:
            parsed = json.loads(raw)
        except json.JSONDecodeError:
            pytest.fail(f"stdout was non-empty but unparseable: {raw!r}")
        if parsed.get("kind") == "acknowledgement":
            ack = parsed.get("acknowledgement", {})
            reasoning_field = ack.get("reasoning", "")
            assert reasoning_field.strip() != "", (
                "bare-ack proposal escaped to stdout — invariant violated"
            )
            assert len(reasoning_field.strip()) >= 40
