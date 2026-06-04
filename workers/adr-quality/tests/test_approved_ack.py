"""TC-321: adr-quality emits approved for an acknowledgement with reasoning >= 40 chars referencing an existing ADR."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

from adr_quality.worker import run_judge

from _helpers import build_bundle, extract_bundle_hash


def _mock_caller_approved_ack(system: str, user: str, model_id: str, max_tokens: int):
    """Mock caller returning an approved verdict for a well-reasoned acknowledgement.

    The verdict cites the acknowledgement rubric criteria (reasoning-substantive,
    governs-feature) and lists both the preflight_gap IRI and the feature_spec
    IRI in `against` per FT-133 / ADR-074.
    """
    bundle_hash = extract_bundle_hash(user)

    verdict = {
        "verdict": "approved",
        "rationale": (
            "The acknowledgement satisfies every acknowledgement-rubric criterion: "
            "the reasoning is substantive (well over 40 characters) and materially "
            "explains why ADR-013 governs this feature; it references an existing "
            "central ADR; it matches the preflight gap; and the proposal is not a "
            "wishful 'almost fits' — ADR-013 genuinely covers the case. Fit for "
            "human acceptance."
        ),
        "judges": "urn:adr-proposal:FT-101",
        "against": [
            "urn:preflight-gap:adr:ADR-013",
            "urn:feature-spec:FT-101",
        ],
        "violates": [],
        "bundle_hash": bundle_hash,
    }
    return json.dumps(verdict), 100, 50


def test_approved_verdict_for_acknowledgement():
    """TC-321: approved verdict for a substantive acknowledgement referencing an existing ADR."""
    bundle = build_bundle(
        bundle_hash="approve3z",
        proposal_iri="urn:adr-proposal:FT-101",
        proposal_kind="acknowledgement",
    )

    # Confirm the acknowledgement under test really has reasoning >= 40 chars
    # (the rubric criterion the judge enforces).
    assert bundle.adr_proposal.acknowledgement is not None
    assert len(bundle.adr_proposal.acknowledgement.reasoning) >= 40

    result = run_judge(bundle, caller=_mock_caller_approved_ack)

    # Approved verdict semantics
    assert result.verdict.verdict == "approved"
    assert len(result.verdict.rationale) >= 20
    assert result.verdict.violates == []
    assert result.verdict.amendment_guidance is None

    # rationale cites acknowledgement-rubric criteria
    rationale_lower = result.verdict.rationale.lower()
    assert "reasoning" in rationale_lower
    assert "adr-013" in rationale_lower or "governs" in rationale_lower

    # ADR-074 polymorphism — `against` contains BOTH the preflight_gap IRI
    # and the feature_spec IRI (FT-133 contract).
    assert "urn:preflight-gap:adr:ADR-013" in result.verdict.against
    assert "urn:feature-spec:FT-101" in result.verdict.against
    assert len(result.verdict.against) == 2

    # Bundle hash echoed byte-for-byte
    assert result.verdict.bundle_hash == "approve3z"


def test_cli_exit_code_zero_for_acknowledgement(tmp_path: Path):
    """TC-321 (exit-code, stdout): worker CLI exits 0 and emits the verdict on stdout."""
    bundle = build_bundle(
        bundle_hash="approve4a",
        proposal_iri="urn:adr-proposal:FT-101",
        proposal_kind="acknowledgement",
    )
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    repo_root = Path(__file__).resolve().parents[3]
    worker_src = repo_root / "workers" / "adr-quality" / "src"
    shared_src = repo_root / "workers" / "_shared" / "src"
    env = os.environ.copy()
    existing_pp = env.get("PYTHONPATH", "")
    env["PYTHONPATH"] = (
        f"{worker_src}:{shared_src}" + (f":{existing_pp}" if existing_pp else "")
    )
    env["ADR_QUALITY_STUB"] = "1"

    proc = subprocess.run(
        [sys.executable, "-m", "adr_quality", "--bundle", str(bundle_path)],
        env=env,
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )

    assert proc.returncode == 0, f"stderr: {proc.stderr}"

    payload = json.loads(proc.stdout.strip())
    assert payload["verdict"] == "approved"
    assert payload["bundle_hash"] == "approve4a"

    # `against` contains both source-of-truth IRIs (gap + spec).
    assert len(payload["against"]) >= 2
