"""TC-316: spec-quality emits approved verdict for a request-faithful schema-conforming SpecProposal."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

from spec_quality.worker import run_judge

from _helpers import build_bundle, extract_bundle_hash


def _mock_caller_approved(system: str, user: str, model_id: str, max_tokens: int):
    """Mock caller that returns an approved verdict.

    Echoes the bundle_hash extracted from the user prompt so the worker's
    bundle_hash invariant holds; the SpecProposal IRI and request IRI are
    used to populate `judges` and `against` so the polymorphism contract
    of ADR-074 is exercised.
    """
    bundle_hash = extract_bundle_hash(user)

    verdict = {
        "verdict": "approved",
        "rationale": (
            "The proposed body contains every required H2 and H3 section "
            "and every behaviour assertion traces to a clause in the originating "
            "request. Bounded, non-colliding, linkage-sound."
        ),
        "judges": "urn:spec-proposal:REQ-007",
        "against": ["urn:request:REQ-007"],
        "violates": [],
        "bundle_hash": bundle_hash,
    }
    return json.dumps(verdict), 100, 50


def test_approved_verdict():
    """TC-316: approved verdict for request-faithful, schema-conforming SpecProposal."""
    bundle = build_bundle(bundle_hash="approve1x", spec_iri="urn:spec-proposal:REQ-007")

    result = run_judge(bundle, caller=_mock_caller_approved)

    # Approved verdict semantics
    assert result.verdict.verdict == "approved"
    assert len(result.verdict.rationale) >= 20
    assert result.verdict.violates == []
    assert result.verdict.amendment_guidance is None

    # ADR-074 polymorphism: judges points at the SpecProposal IRI;
    # against carries exactly the originating request IRI.
    assert result.verdict.judges == "urn:spec-proposal:REQ-007"
    assert result.verdict.against == ["urn:request:REQ-007"]

    # Bundle hash echoed byte-for-byte (FT-132 / ADR-073 protocol).
    assert result.verdict.bundle_hash == "approve1x"


def test_cli_exit_code_zero_and_stdout_carries_verdict_json(tmp_path: Path):
    """TC-316 (exit-code, stdout): the worker CLI exits 0 and emits the verdict on stdout."""
    bundle = build_bundle(bundle_hash="approve2y", spec_iri="urn:spec-proposal:REQ-007")
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    repo_root = Path(__file__).resolve().parents[3]
    worker_src = repo_root / "workers" / "spec-quality" / "src"
    shared_src = repo_root / "workers" / "_shared" / "src"
    env = os.environ.copy()
    existing_pp = env.get("PYTHONPATH", "")
    env["PYTHONPATH"] = (
        f"{worker_src}:{shared_src}" + (f":{existing_pp}" if existing_pp else "")
    )
    env["SPEC_QUALITY_STUB"] = "1"

    proc = subprocess.run(
        [sys.executable, "-m", "spec_quality", "--bundle", str(bundle_path)],
        env=env,
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )

    # exit-code observable: 0 on a successful verdict emission.
    assert proc.returncode == 0, f"stderr: {proc.stderr}"

    # stdout observable: a single JSON QualityVerdict object.
    payload = json.loads(proc.stdout.strip())
    assert payload["verdict"] == "approved"
    assert payload["bundle_hash"] == "approve2y"
    assert payload["judges"]
    assert payload["against"]
