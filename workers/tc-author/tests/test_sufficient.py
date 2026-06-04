"""TC-286 — tc-author returns sufficient when feature already meets min_tcs_per_feature.

Acceptance criteria (from FT-126 / TC-286):

- Parsed stdout deserialises to a TcProposal whose kind field equals "sufficient".
- The proposal's coverage_map is present and non-empty.
- Neither the new payload nor the augment payload is populated.
- The worker exits with status code 0.
- The proposal's bundle_hash field equals the bundle_hash of the synthetic input bundle.
"""

from __future__ import annotations

import io
import json
import sys
from pathlib import Path

from _helpers import (
    BUNDLE_HASH,
    assert_no_anthropic_attempt,
    build_bundle_for_sufficient,
    make_caller,
)


def test_tc_286_run_author_returns_sufficient(monkeypatch) -> None:
    """Worker-API path: inject a mocked caller and verify the proposal shape."""
    from tc_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_sufficient()
    caller = make_caller(
        {
            "kind": "sufficient",
            "bundle_hash": bundle.bundle_hash,
            "sufficient": {
                "reasoning": "Existing three TCs already meet target_count of 3 and cover all required axes.",
                "coverage_map": {
                    "TC-001": ["happy"],
                    "TC-002": ["edge"],
                    "TC-003": ["integration"],
                },
            },
        }
    )

    result = run_author(bundle, caller=caller)

    assert result.proposal.kind == "sufficient"
    assert result.proposal.sufficient is not None
    assert len(result.proposal.sufficient.coverage_map) > 0
    assert result.proposal.bundle_hash == bundle.bundle_hash == BUNDLE_HASH
    assert result.proposal.new is None
    assert result.proposal.augment is None
    assert result.telemetry.attempts == 1
    assert len(caller.calls) == 1


def test_tc_286_cli_round_trip(monkeypatch, tmp_path) -> None:
    """CLI path: write the bundle to a file, drive __main__, parse stdout."""
    from tc_author import __main__ as cli
    from tc_author import worker as worker_mod
    from tc_author.output import TcProposal

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_sufficient()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(
        {
            "kind": "sufficient",
            "bundle_hash": bundle.bundle_hash,
            "sufficient": {
                "reasoning": "Existing TCs meet target_count and cover all axes.",
                "coverage_map": {
                    "TC-001": ["happy"],
                    "TC-002": ["edge"],
                    "TC-003": ["integration"],
                },
            },
        }
    )

    # Patch the default caller resolver so run_author uses our mock.
    monkeypatch.setattr(worker_mod, "_stub_caller", caller)
    monkeypatch.setenv("TC_AUTHOR_STUB", "1")

    # Snapshot tmp_path contents to verify no extra writes happen.
    before = set(Path(tmp_path).rglob("*"))

    stdout = io.StringIO()
    stderr = io.StringIO()
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(sys, "stderr", stderr)

    code = cli.main(["--bundle", str(bundle_path)])

    assert code == 0, f"expected exit 0; stderr={stderr.getvalue()}"

    raw = stdout.getvalue().strip()
    assert raw, "stdout must contain a TcProposal JSON object"
    assert "\n" not in raw, "stdout proposal must be a single line of JSON"

    proposal = TcProposal.model_validate_json(raw)
    assert proposal.kind == "sufficient"
    assert proposal.sufficient is not None
    assert len(proposal.sufficient.coverage_map) > 0
    assert proposal.bundle_hash == bundle.bundle_hash

    # The worker is forbidden from filesystem writes outside stdout.
    after = set(Path(tmp_path).rglob("*"))
    assert after == before, f"worker wrote unexpected files: {after - before}"


def test_tc_286_proposal_serialisation_round_trips() -> None:
    """The proposal JSON must round-trip through TcProposal cleanly."""
    from tc_author.output import TcProposal

    raw = json.dumps(
        {
            "kind": "sufficient",
            "bundle_hash": BUNDLE_HASH,
            "sufficient": {
                "reasoning": "Existing TCs meet target_count and cover axes.",
                "coverage_map": {"TC-001": ["happy"], "TC-002": ["edge"]},
            },
        }
    )
    proposal = TcProposal.model_validate_json(raw)
    redumped = json.loads(proposal.model_dump_json(exclude_none=True))
    assert redumped["kind"] == "sufficient"
    assert redumped["bundle_hash"] == BUNDLE_HASH
    assert "coverage_map" in redumped["sufficient"]
    assert "new" not in redumped
    assert "augment" not in redumped
