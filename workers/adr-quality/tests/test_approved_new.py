"""TC-320: adr-quality emits approved for a new ADR that schema-conforms and soundly closes the preflight gap."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

from adr_quality.worker import run_judge

from _helpers import build_bundle, extract_bundle_hash


def _mock_caller_approved(system: str, user: str, model_id: str, max_tokens: int):
    """Mock caller that returns an approved verdict.

    Echoes the bundle_hash extracted from the user prompt so the worker's
    bundle_hash invariant holds; populates `judges` and `against` so the
    polymorphism contract of ADR-074 is exercised. The rationale walks
    the five new-ADR rubric criteria.
    """
    bundle_hash = extract_bundle_hash(user)

    verdict = {
        "verdict": "approved",
        "rationale": (
            "The proposal clears every new-ADR criterion: schema-conforming "
            "(all five H2 sections present), gap-closing (the Decision section "
            "materially closes the unacknowledged-ADR gap), scope-correct "
            "(cross-cutting matches the gap kind), domain-valid (proposed_domains "
            "are members of the registry), and alternatives-noted (two substantive "
            "alternatives with rationale). The proposal traceably addresses the "
            "preflight gap and is fit for human review."
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


def test_approved_verdict_for_new_adr():
    """TC-320: approved verdict for schema-conforming, gap-closing new-ADR proposal."""
    bundle = build_bundle(
        bundle_hash="approve1x",
        proposal_iri="urn:adr-proposal:FT-101",
        proposal_kind="new",
        proposal_scope="cross-cutting",
    )

    result = run_judge(bundle, caller=_mock_caller_approved)

    # Approved verdict semantics
    assert result.verdict.verdict == "approved"
    assert len(result.verdict.rationale) >= 20
    assert result.verdict.violates == []
    assert result.verdict.amendment_guidance is None

    # rationale walks the five new-ADR rubric criteria
    rationale_lower = result.verdict.rationale.lower()
    assert "schema-conforming" in rationale_lower
    assert "gap-closing" in rationale_lower
    assert "scope-correct" in rationale_lower
    assert "alternatives-noted" in rationale_lower

    # ADR-074 polymorphism: judges points at the AdrProposal IRI;
    # against carries both the preflight_gap IRI and the feature_spec IRI.
    assert result.verdict.judges == "urn:adr-proposal:FT-101"
    assert "urn:preflight-gap:adr:ADR-013" in result.verdict.against
    assert "urn:feature-spec:FT-101" in result.verdict.against

    # Bundle hash echoed byte-for-byte (FT-133 / ADR-073 protocol).
    assert result.verdict.bundle_hash == "approve1x"


def test_cli_exit_code_zero_and_stdout_carries_verdict_json(tmp_path: Path):
    """TC-320 (exit-code, stdout): the worker CLI exits 0 and emits the verdict on stdout."""
    bundle = build_bundle(
        bundle_hash="approve2y",
        proposal_iri="urn:adr-proposal:FT-101",
        proposal_kind="new",
        proposal_scope="cross-cutting",
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

    # exit-code observable: 0 on a successful verdict emission.
    assert proc.returncode == 0, f"stderr: {proc.stderr}"

    # stdout observable: a single JSON QualityVerdict object.
    payload = json.loads(proc.stdout.strip())
    assert payload["verdict"] == "approved"
    assert payload["bundle_hash"] == "approve2y"
    assert payload["judges"]
    assert payload["against"]
