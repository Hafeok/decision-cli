"""TC-078 — verify-graph-author worker echoes bundle_hash for protocol integrity.

Acceptance criteria (from FT-048 / TC-078):

- The worker's internal validation detects the hash mismatch.
- The worker exits with non-zero (5 per FT-048's error table) and a structured
  stderr message identifying the mismatch.
- No GraphProposal is written to stdout (or any output is marked as invalid).
"""

from __future__ import annotations

import io
import sys

from _helpers import (
    assert_no_anthropic_attempt,
    build_bundle_for_protocol_check,
    make_caller,
)


def test_tc_078_worker_raises_on_hash_mismatch(monkeypatch) -> None:
    """Direct worker API: BundleHashMismatch is raised when the proposal echoes a wrong hash."""
    from verify_graph_author.worker import BundleHashMismatch, run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_protocol_check()
    # The mocked model returns a `new` proposal with a wrong-on-purpose hash.
    caller = make_caller(
        {
            "kind": "new",
            "bundle_hash": "wrong-hash-aaaaaaaa",  # stale / corrupted echo
            "new": {
                "environment": "ENV-1",
                "steps": [
                    {
                        "step_type": "wait-for",
                        "fields": {"condition": "ready"},
                        "provides_evidence_for": ["T1"],
                    }
                ],
                "rationale": "Wait until the system reports ready.",
            },
        }
    )

    raised = False
    try:
        run_author(bundle, caller=caller)
    except BundleHashMismatch as exc:
        raised = True
        message = str(exc)
        assert "wrong-hash-aaaaaaaa" in message
        assert bundle.bundle_hash in message
    assert raised, "worker must raise BundleHashMismatch when the proposal echoes a wrong hash"


def test_tc_078_cli_exits_five_on_hash_mismatch(monkeypatch, tmp_path) -> None:
    """CLI path: exit code 5 per FT-048 §Error table, structured stderr, no stdout proposal."""
    from verify_graph_author import __main__ as cli
    from verify_graph_author import worker as worker_mod

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_protocol_check()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(
        {
            "kind": "new",
            "bundle_hash": "wrong-hash-aaaaaaaa",
            "new": {
                "environment": "ENV-1",
                "steps": [
                    {
                        "step_type": "wait-for",
                        "fields": {"condition": "ready"},
                        "provides_evidence_for": ["T1"],
                    }
                ],
                "rationale": "Wait until ready.",
            },
        }
    )
    monkeypatch.setattr(worker_mod, "_stub_caller", caller)
    monkeypatch.setenv("VERIFY_GRAPH_AUTHOR_STUB", "1")

    stdout = io.StringIO()
    stderr = io.StringIO()
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(sys, "stderr", stderr)

    code = cli.main(["--bundle", str(bundle_path)])

    assert code == 5, f"hash mismatch must exit 5; got {code}; stderr={stderr.getvalue()}"
    assert stdout.getvalue() == "", "no GraphProposal must be written on hash mismatch"
    err = stderr.getvalue()
    assert "bundle_hash mismatch" in err
    assert "wrong-hash-aaaaaaaa" in err
    assert bundle.bundle_hash in err


def test_tc_078_correct_hash_proposal_succeeds(monkeypatch, tmp_path) -> None:
    """Sanity: with the correct hash echoed, the same shape exits 0."""
    from verify_graph_author import __main__ as cli
    from verify_graph_author import worker as worker_mod
    from verify_graph_author.output import GraphProposal

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_protocol_check()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(
        {
            "kind": "new",
            "bundle_hash": bundle.bundle_hash,
            "new": {
                "environment": "ENV-1",
                "steps": [
                    {
                        "step_type": "wait-for",
                        "fields": {"condition": "ready"},
                        "provides_evidence_for": ["T1"],
                    }
                ],
                "rationale": "Wait until ready, the simplest possible covering step.",
            },
        }
    )
    monkeypatch.setattr(worker_mod, "_stub_caller", caller)
    monkeypatch.setenv("VERIFY_GRAPH_AUTHOR_STUB", "1")

    stdout = io.StringIO()
    stderr = io.StringIO()
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(sys, "stderr", stderr)

    code = cli.main(["--bundle", str(bundle_path)])
    assert code == 0, f"matching hash must exit 0; stderr={stderr.getvalue()}"

    proposal = GraphProposal.model_validate_json(stdout.getvalue().strip())
    assert proposal.bundle_hash == bundle.bundle_hash
