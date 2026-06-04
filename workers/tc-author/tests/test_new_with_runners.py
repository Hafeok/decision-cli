"""TC-287 — tc-author returns new with target_count TCs and wired runner fields.

Acceptance criteria (from FT-126 / TC-287):

- Parsed stdout deserialises to a TcProposal whose kind equals "new".
- new.tcs.len() equals exactly 4 (the target_count supplied in the bundle).
- Every tc.runner_args is a non-empty string.
- Every tc.runner is in the allowed runner enum.
- Every tc.observes array is non-empty.
- The worker exits with status code 0.
"""

from __future__ import annotations

import io
import json
import sys
from pathlib import Path

from _helpers import (
    BUNDLE_HASH,
    assert_no_anthropic_attempt,
    build_bundle_for_new,
    make_caller,
)


def test_tc_287_run_author_returns_new_with_runners(monkeypatch) -> None:
    """Worker-API path: inject a mocked caller and verify the proposal shape."""
    from tc_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_new()
    caller = make_caller(
        {
            "kind": "new",
            "bundle_hash": bundle.bundle_hash,
            "new": {
                "replaced_tcs": [],
                "tcs": [
                    {
                        "title": "Happy path test",
                        "type": "scenario",
                        "body": "## Purpose\n\nTests happy path.\n\n## Acceptance\n\n- Feature works.\n",
                        "runner": "bash",
                        "runner_args": "tests/scripts/tc-001.sh",
                        "runner_timeout": "60s",
                        "observes": ["CLI command surface"],
                        "coverage_axis": "happy",
                        "validates_features": ["FT-stub"],
                        "validates_adrs": [],
                    },
                    {
                        "title": "Edge case test",
                        "type": "scenario",
                        "body": "## Purpose\n\nTests edge case.\n\n## Acceptance\n\n- Handles edge.\n",
                        "runner": "cargo-test",
                        "runner_args": "test_edge_case",
                        "runner_timeout": "120s",
                        "observes": ["Error handling"],
                        "coverage_axis": "edge",
                        "validates_features": ["FT-stub"],
                        "validates_adrs": [],
                    },
                    {
                        "title": "Integration test",
                        "type": "scenario",
                        "body": "## Purpose\n\nTests integration.\n\n## Acceptance\n\n- Integrates.\n",
                        "runner": "pytest",
                        "runner_args": "tests/test_integration.py::test_x",
                        "runner_timeout": "60s",
                        "observes": ["API surface"],
                        "coverage_axis": "integration",
                        "validates_features": ["FT-stub"],
                        "validates_adrs": [],
                    },
                    {
                        "title": "State persistence test",
                        "type": "scenario",
                        "body": "## Purpose\n\nTests state.\n\n## Acceptance\n\n- State persists.\n",
                        "runner": "bash",
                        "runner_args": "tests/scripts/tc-004.sh",
                        "runner_timeout": "30s",
                        "observes": ["Graph mutations"],
                        "coverage_axis": "state",
                        "validates_features": ["FT-stub"],
                        "validates_adrs": [],
                    },
                ],
                "reasoning": "These four TCs cover FT-stub across all axes: happy, edge, integration, state.",
            },
        }
    )

    result = run_author(bundle, caller=caller)

    assert result.proposal.kind == "new"
    assert result.proposal.new is not None
    assert len(result.proposal.new.tcs) == 4
    for tc in result.proposal.new.tcs:
        assert len(tc.runner_args) > 0, f"TC '{tc.title}' has empty runner_args"
        assert tc.runner in ["bash", "cargo-test", "pytest", "custom"]
        assert len(tc.observes) > 0, f"TC '{tc.title}' has empty observes"
    assert result.proposal.bundle_hash == bundle.bundle_hash == BUNDLE_HASH
    assert result.proposal.sufficient is None
    assert result.proposal.augment is None
    assert result.telemetry.attempts == 1


def test_tc_287_cli_round_trip(monkeypatch, tmp_path) -> None:
    """CLI path: write the bundle to a file, drive __main__, parse stdout."""
    from tc_author import __main__ as cli
    from tc_author import worker as worker_mod
    from tc_author.output import TcProposal

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_new()
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
                        "runner": "bash",
                        "runner_args": "tests/scripts/tc-1.sh",
                        "runner_timeout": "60s",
                        "observes": ["CLI"],
                        "coverage_axis": "happy",
                        "validates_features": ["FT-stub"],
                        "validates_adrs": [],
                    },
                    {
                        "title": "TC2",
                        "type": "scenario",
                        "body": "## Purpose\n\nTC2.\n",
                        "runner": "cargo-test",
                        "runner_args": "test_two",
                        "runner_timeout": "120s",
                        "observes": ["API"],
                        "coverage_axis": "edge",
                        "validates_features": ["FT-stub"],
                        "validates_adrs": [],
                    },
                    {
                        "title": "TC3",
                        "type": "scenario",
                        "body": "## Purpose\n\nTC3.\n",
                        "runner": "pytest",
                        "runner_args": "tests/test_three.py::test_x",
                        "runner_timeout": "60s",
                        "observes": ["Integration"],
                        "coverage_axis": "integration",
                        "validates_features": ["FT-stub"],
                        "validates_adrs": [],
                    },
                    {
                        "title": "TC4",
                        "type": "scenario",
                        "body": "## Purpose\n\nTC4.\n",
                        "runner": "bash",
                        "runner_args": "tests/scripts/tc-4.sh",
                        "runner_timeout": "30s",
                        "observes": ["State"],
                        "coverage_axis": "state",
                        "validates_features": ["FT-stub"],
                        "validates_adrs": [],
                    },
                ],
                "reasoning": "Four TCs covering all axes.",
            },
        }
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
    assert proposal.kind == "new"
    assert proposal.new is not None
    assert len(proposal.new.tcs) == 4
    for tc in proposal.new.tcs:
        assert len(tc.runner_args) > 0
        assert tc.runner in ["bash", "cargo-test", "pytest", "custom"]
        assert len(tc.observes) > 0

    after = set(Path(tmp_path).rglob("*"))
    assert after == before, f"worker wrote unexpected files: {after - before}"
