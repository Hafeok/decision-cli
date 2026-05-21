"""TC-076 — verify-graph-author worker returns Match when a candidate covers all TCs.

Acceptance criteria (from FT-048 / TC-076):

- The worker exits 0.
- stdout contains a single line of JSON parseable as GraphProposal with kind == "match".
- proposal.match.graph_id == "VG-K".
- proposal.bundle_hash echoes the input's bundle_hash exactly.
- The worker performs no filesystem writes outside stdout.
- No Anthropic API call escapes the mock.
"""

from __future__ import annotations

import io
import json
import os
import sys
from pathlib import Path

from _helpers import (
    BUNDLE_HASH,
    assert_no_anthropic_attempt,
    build_bundle_for_match,
    make_caller,
)


def test_tc_076_run_author_returns_match(monkeypatch) -> None:
    """Worker-API path: inject a mocked caller and verify the proposal shape."""
    from verify_graph_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_match()
    caller = make_caller(
        {
            "kind": "match",
            "bundle_hash": bundle.bundle_hash,
            "match": {
                "graph_id": "VG-K",
                "rationale": "VG-K already covers both TCs through the shell+file-assertion pair.",
            },
        }
    )

    result = run_author(bundle, caller=caller)

    assert result.proposal.kind == "match"
    assert result.proposal.match is not None
    assert result.proposal.match.graph_id == "VG-K"
    assert result.proposal.bundle_hash == bundle.bundle_hash == BUNDLE_HASH
    assert result.proposal.new is None
    assert result.proposal.gap is None
    assert result.telemetry.attempts == 1
    assert len(caller.calls) == 1


def test_tc_076_cli_round_trip(monkeypatch, tmp_path) -> None:
    """CLI path: write the bundle to a file, drive __main__, parse stdout."""
    from verify_graph_author import __main__ as cli
    from verify_graph_author import worker as worker_mod
    from verify_graph_author.output import GraphProposal

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_match()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(
        {
            "kind": "match",
            "bundle_hash": bundle.bundle_hash,
            "match": {
                "graph_id": "VG-K",
                "rationale": "VG-K already covers T1 and T2 with the shell + file-assertion pair.",
            },
        }
    )

    # Patch the default caller resolver so run_author uses our mock without us
    # threading caller= through the CLI surface.
    monkeypatch.setattr(worker_mod, "_stub_caller", caller)
    monkeypatch.setenv("VERIFY_GRAPH_AUTHOR_STUB", "1")

    # Snapshot tmp_path contents to verify no extra writes happen.
    before = set(Path(tmp_path).rglob("*"))

    stdout = io.StringIO()
    stderr = io.StringIO()
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(sys, "stderr", stderr)

    code = cli.main(["--bundle", str(bundle_path)])

    assert code == 0, f"expected exit 0; stderr={stderr.getvalue()}"

    raw = stdout.getvalue().strip()
    assert raw, "stdout must contain a GraphProposal JSON object"
    assert "\n" not in raw, "stdout proposal must be a single line of JSON"

    proposal = GraphProposal.model_validate_json(raw)
    assert proposal.kind == "match"
    assert proposal.match is not None
    assert proposal.match.graph_id == "VG-K"
    assert proposal.bundle_hash == bundle.bundle_hash

    # The worker is forbidden from filesystem writes outside stdout —
    # tmp_path should contain only the original bundle file.
    after = set(Path(tmp_path).rglob("*"))
    assert after == before, f"worker wrote unexpected files: {after - before}"


def test_tc_076_proposal_serialisation_round_trips() -> None:
    """The proposal JSON must round-trip through GraphProposal cleanly."""
    from verify_graph_author.output import GraphProposal

    raw = json.dumps(
        {
            "kind": "match",
            "bundle_hash": BUNDLE_HASH,
            "match": {
                "graph_id": "VG-K",
                "rationale": "Both TCs are covered by VG-K's shell+file-assertion sequence.",
            },
        }
    )
    proposal = GraphProposal.model_validate_json(raw)
    redumped = json.loads(proposal.model_dump_json(exclude_none=True))
    assert redumped["kind"] == "match"
    assert redumped["bundle_hash"] == BUNDLE_HASH
    assert redumped["match"]["graph_id"] == "VG-K"
    assert "new" not in redumped
    assert "gap" not in redumped
