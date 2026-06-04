"""TC-288 — tc-author falls back to sufficient when proposed runners fail validation.

Acceptance criteria (from FT-126 / TC-288):

- Parsed stdout deserialises to a TcProposal whose kind equals "sufficient".
- The proposal's reasoning string contains the substring "could not produce wireable".
- Neither the new payload nor the augment payload is populated.
- The worker exits with status code 0 (graceful fallback, not error).
- The mocked Anthropic client recorded exactly two calls (initial attempt + one retry).
"""

from __future__ import annotations

import io
import json
import sys
from pathlib import Path

from _helpers import (
    BUNDLE_HASH,
    assert_no_anthropic_attempt,
    build_bundle_for_runner_args_fallback,
    make_caller,
)


def test_tc_288_run_author_falls_back_to_sufficient(monkeypatch) -> None:
    """Worker-API path: inject a caller that returns bad runner_args twice."""
    from tc_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_runner_args_fallback()
    # First attempt and retry both return malformed runner_args
    caller = make_caller(
        {
            "kind": "new",
            "bundle_hash": bundle.bundle_hash,
            "new": {
                "replaced_tcs": [],
                "tcs": [
                    {
                        "title": "TC1",
                        "type": "scenario",
                        "body": "## Purpose\n\nTC1.\n",
                        "runner": "bash",
                        "runner_args": "",  # Empty runner_args — invalid!
                        "runner_timeout": "60s",
                        "observes": ["CLI"],
                        "coverage_axis": "happy",
                        "validates_features": ["FT-stub"],
                        "validates_adrs": [],
                    },
                ],
                "reasoning": "TCs for FT-stub.",
            },
        },
        {
            "kind": "new",
            "bundle_hash": bundle.bundle_hash,
            "new": {
                "replaced_tcs": [],
                "tcs": [
                    {
                        "title": "TC1",
                        "type": "scenario",
                        "body": "## Purpose\n\nTC1.\n",
                        "runner": "bash",
                        "runner_args": "no-dot-sh-extension",  # Doesn't match pattern .+\.sh$
                        "runner_timeout": "60s",
                        "observes": ["CLI"],
                        "coverage_axis": "happy",
                        "validates_features": ["FT-stub"],
                        "validates_adrs": [],
                    },
                ],
                "reasoning": "TCs for FT-stub (retry).",
            },
        },
    )

    result = run_author(bundle, caller=caller)

    assert result.proposal.kind == "sufficient"
    assert result.proposal.sufficient is not None
    assert "could not produce wireable" in result.proposal.sufficient.reasoning
    assert result.proposal.new is None
    assert result.proposal.augment is None
    assert result.telemetry.attempts == 2
    assert len(caller.calls) == 2


def test_tc_288_cli_round_trip(monkeypatch, tmp_path) -> None:
    """CLI path: verify fallback and attempt count via CLI."""
    from tc_author import __main__ as cli
    from tc_author import worker as worker_mod
    from tc_author.output import TcProposal

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_runner_args_fallback()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(
        {
            "kind": "new",
            "bundle_hash": bundle.bundle_hash,
            "new": {
                "replaced_tcs": [],
                "tcs": [
                    {
                        "title": "TC1",
                        "type": "scenario",
                        "body": "## Purpose\n\nTC1.\n",
                        "runner": "cargo-test",
                        "runner_args": "invalid-runner-args-with-dashes!",  # Doesn't match ^[a-zA-Z_][a-zA-Z0-9_]*$
                        "runner_timeout": "60s",
                        "observes": ["CLI"],
                        "validates_features": ["FT-stub"],
                        "validates_adrs": [],
                    },
                ],
                "reasoning": "TCs for FT-stub.",
            },
        },
        {
            "kind": "new",
            "bundle_hash": bundle.bundle_hash,
            "new": {
                "replaced_tcs": [],
                "tcs": [
                    {
                        "title": "TC1",
                        "type": "scenario",
                        "body": "## Purpose\n\nTC1.\n",
                        "runner": "bash",
                        "runner_args": "",  # Empty — invalid!
                        "runner_timeout": "60s",
                        "observes": ["CLI"],
                        "validates_features": ["FT-stub"],
                        "validates_adrs": [],
                    },
                ],
                "reasoning": "TCs for FT-stub (retry).",
            },
        },
    )

    monkeypatch.setattr(worker_mod, "_stub_caller", caller)
    monkeypatch.setenv("TC_AUTHOR_STUB", "1")

    before = set(Path(tmp_path).rglob("*"))

    stdout = io.StringIO()
    stderr = io.StringIO()
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(sys, "stderr", stderr)

    code = cli.main(["--bundle", str(bundle_path)])

    assert code == 0, f"expected exit 0; stderr={stderr.getvalue()}"

    raw = stdout.getvalue().strip()
    assert raw, "stdout must contain a TcProposal JSON object"

    proposal = TcProposal.model_validate_json(raw)
    assert proposal.kind == "sufficient"
    assert proposal.sufficient is not None
    assert "could not produce wireable" in proposal.sufficient.reasoning
    assert proposal.new is None
    assert proposal.augment is None

    # Verify two attempts were made
    assert len(caller.calls) == 2

    after = set(Path(tmp_path).rglob("*"))
    assert after == before, f"worker wrote unexpected files: {after - before}"
