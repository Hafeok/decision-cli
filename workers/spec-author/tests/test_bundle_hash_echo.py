"""TC-301 — spec-author echoes bundle_hash and writes nothing beyond stdout.

Acceptance criteria (from FT-129 / TC-301):

- Parsed stdout SpecProposal.bundle_hash equals the input bundle's bundle_hash, byte for byte.
- No file is created, modified, or deleted in CWD during worker execution.
- No file is created, modified, or deleted under the test's tmp dir namespace.
- The worker exits with status code 0.
"""

from __future__ import annotations

import io
import os
import sys
from pathlib import Path

from _helpers import (
    CONFORMING_BODY,
    assert_no_anthropic_attempt,
    build_bundle_for_protocol_check,
    make_caller,
)


def _approved_new_response(bundle_hash: str) -> dict:
    return {
        "kind": "new",
        "bundle_hash": bundle_hash,
        "new": {
            "title": "Protocol-check worker",
            "body": CONFORMING_BODY,
            "proposed_depends_on": [],
            "proposed_adrs": ["ADR-073"],
            "proposed_domains": [],
            "rationale": (
                "A trivial body that satisfies the body-completeness contract "
                "so the test can focus on bundle_hash echo and disk state."
            ),
        },
    }


def test_tc_301_bundle_hash_echo(monkeypatch) -> None:
    """Worker echoes the input bundle_hash exactly."""
    from spec_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_protocol_check()
    caller = make_caller(_approved_new_response(bundle.bundle_hash))

    result = run_author(bundle, caller=caller)

    assert result.proposal.bundle_hash == bundle.bundle_hash


def test_tc_301_no_filesystem_writes(monkeypatch, tmp_path) -> None:
    """Worker performs no filesystem writes outside stdout."""
    from spec_author import __main__ as cli
    from spec_author import worker as worker_mod

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_protocol_check()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(_approved_new_response(bundle.bundle_hash))

    monkeypatch.setattr(worker_mod, "_stub_caller", caller)
    monkeypatch.setenv("SPEC_AUTHOR_STUB", "1")

    # Snapshot tmp_path contents before invocation.
    before_tmp = set(Path(tmp_path).rglob("*"))

    # Snapshot CWD contents before invocation (top-level only — CWD may
    # be the repo root and rglob'ing the whole tree is too noisy for the
    # assertion we want to make).
    cwd = Path(os.getcwd())
    before_cwd = set(cwd.iterdir()) if cwd.exists() else set()

    stdout = io.StringIO()
    stderr = io.StringIO()
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(sys, "stderr", stderr)

    code = cli.main(["--bundle", str(bundle_path)])

    assert code == 0, f"expected exit 0; stderr={stderr.getvalue()}"

    # Verify no new files were created under tmp_path.
    after_tmp = set(Path(tmp_path).rglob("*"))
    assert after_tmp == before_tmp, (
        f"worker wrote unexpected files under tmp_path: {after_tmp - before_tmp}"
    )

    # Verify CWD top-level contents unchanged.
    after_cwd = set(cwd.iterdir()) if cwd.exists() else set()
    new_in_cwd = after_cwd - before_cwd
    assert not new_in_cwd, f"worker created unexpected entries in CWD: {new_in_cwd}"

    # Verify the bundle_hash was echoed correctly in stdout.
    raw = stdout.getvalue().strip()
    from spec_author.output import SpecProposal

    proposal = SpecProposal.model_validate_json(raw)
    assert proposal.bundle_hash == bundle.bundle_hash


def test_tc_301_bundle_hash_mismatch_detected(monkeypatch) -> None:
    """Worker detects and rejects a bundle_hash mismatch (protocol violation)."""
    from spec_author.worker import BundleHashMismatch, run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_protocol_check()
    wrong_hash = "wrong_hash_" + ("0" * 54)
    caller = make_caller(
        {
            "kind": "gap",
            "bundle_hash": wrong_hash,
            "gap": {
                "missing_information": ["intentional"],
                "reason": "Intentional bundle_hash mismatch for testing.",
            },
        }
    )

    try:
        run_author(bundle, caller=caller)
        raise AssertionError("Expected BundleHashMismatch to be raised")
    except BundleHashMismatch as exc:
        assert bundle.bundle_hash in str(exc)
        assert wrong_hash in str(exc)


def test_tc_301_positional_bundle_path(monkeypatch, tmp_path) -> None:
    """The CLI also accepts the bundle path positionally (FT-129 invocation)."""
    from spec_author import __main__ as cli
    from spec_author import worker as worker_mod
    from spec_author.output import SpecProposal

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_protocol_check()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(_approved_new_response(bundle.bundle_hash))
    monkeypatch.setattr(worker_mod, "_stub_caller", caller)
    monkeypatch.setenv("SPEC_AUTHOR_STUB", "1")

    stdout = io.StringIO()
    stderr = io.StringIO()
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(sys, "stderr", stderr)

    # Positional bundle-path form: `python -m spec_author <path>`
    code = cli.main([str(bundle_path)])
    assert code == 0, f"expected exit 0; stderr={stderr.getvalue()}"

    proposal = SpecProposal.model_validate_json(stdout.getvalue().strip())
    assert proposal.bundle_hash == bundle.bundle_hash
