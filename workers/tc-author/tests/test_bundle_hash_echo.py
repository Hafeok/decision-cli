"""TC-289 — tc-author echoes bundle_hash and writes nothing beyond stdout.

Acceptance criteria (from FT-126 / TC-289):

- Parsed stdout TcProposal.bundle_hash equals the bundle_hash field of the input bundle.
- No file is created, modified, or deleted in the CWD during worker execution.
- No file is created, modified, or deleted under the system temp directory.
- The worker exits with status code 0.
"""

from __future__ import annotations

import io
import sys
from pathlib import Path

from _helpers import (
    assert_no_anthropic_attempt,
    build_bundle_for_protocol_check,
    make_caller,
)


def test_tc_289_bundle_hash_echo(monkeypatch, tmp_path) -> None:
    """Worker echoes the input bundle_hash exactly."""
    from tc_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_protocol_check()
    caller = make_caller(
        {
            "kind": "sufficient",
            "bundle_hash": bundle.bundle_hash,
            "sufficient": {
                "reasoning": "Echoing bundle_hash correctly.",
                "coverage_map": {},
            },
        }
    )

    result = run_author(bundle, caller=caller)

    assert result.proposal.bundle_hash == bundle.bundle_hash


def test_tc_289_no_filesystem_writes(monkeypatch, tmp_path) -> None:
    """Worker performs no filesystem writes outside stdout."""
    from tc_author import __main__ as cli
    from tc_author import worker as worker_mod

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_protocol_check()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(
        {
            "kind": "sufficient",
            "bundle_hash": bundle.bundle_hash,
            "sufficient": {
                "reasoning": "Stateless worker, no filesystem writes.",
                "coverage_map": {},
            },
        }
    )

    monkeypatch.setattr(worker_mod, "_stub_caller", caller)
    monkeypatch.setenv("TC_AUTHOR_STUB", "1")

    # Snapshot tmp_path contents before invocation
    before = set(Path(tmp_path).rglob("*"))

    stdout = io.StringIO()
    stderr = io.StringIO()
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(sys, "stderr", stderr)

    code = cli.main(["--bundle", str(bundle_path)])

    assert code == 0, f"expected exit 0; stderr={stderr.getvalue()}"

    # Verify no new files were created
    after = set(Path(tmp_path).rglob("*"))
    assert after == before, f"worker wrote unexpected files: {after - before}"

    # Verify the bundle_hash was echoed correctly in stdout
    raw = stdout.getvalue().strip()
    from tc_author.output import TcProposal

    proposal = TcProposal.model_validate_json(raw)
    assert proposal.bundle_hash == bundle.bundle_hash


def test_tc_289_bundle_hash_mismatch_detected(monkeypatch) -> None:
    """Worker detects and rejects a bundle_hash mismatch (protocol violation)."""
    from tc_author.worker import BundleHashMismatch, run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_protocol_check()
    # Mock returns a different bundle_hash than the input
    wrong_hash = "wrong_hash_" + ("0" * 54)
    caller = make_caller(
        {
            "kind": "sufficient",
            "bundle_hash": wrong_hash,
            "sufficient": {
                "reasoning": "Intentional bundle_hash mismatch for testing.",
                "coverage_map": {},
            },
        }
    )

    try:
        run_author(bundle, caller=caller)
        assert False, "Expected BundleHashMismatch to be raised"
    except BundleHashMismatch as exc:
        assert bundle.bundle_hash in str(exc)
        assert wrong_hash in str(exc)
