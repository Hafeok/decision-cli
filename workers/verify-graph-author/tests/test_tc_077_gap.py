"""TC-077 — verify-graph-author worker returns Gap when allowed_ops are insufficient.

Acceptance criteria (from FT-048 / TC-077):

- The worker exits 0 (Gap is a valid outcome, not a fault).
- stdout's GraphProposal.kind == "gap".
- proposal.gap.uncovered_tcs == ["T1"].
- proposal.gap.reason is non-empty and mentions the ops mismatch.
- The worker does NOT invent a synthetic step using http-readonly to fake coverage.
"""

from __future__ import annotations

import io
import sys

from _helpers import (
    assert_no_anthropic_attempt,
    build_bundle_for_gap_ops_mismatch,
    make_caller,
)


def test_tc_077_run_author_returns_gap(monkeypatch) -> None:
    """Worker-API path: mocked Claude returns Gap citing the ops mismatch."""
    from verify_graph_author.worker import run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_gap_ops_mismatch()
    caller = make_caller(
        {
            "kind": "gap",
            "bundle_hash": bundle.bundle_hash,
            "gap": {
                "uncovered_tcs": ["T1"],
                "reason": (
                    "TC requires http-mutating but target environment allows "
                    "only http-readonly; no available step kind can produce "
                    "evidence for the POST side-effect without violating "
                    "allowed_ops."
                ),
            },
        }
    )

    result = run_author(bundle, caller=caller)

    assert result.proposal.kind == "gap"
    assert result.proposal.gap is not None
    assert result.proposal.gap.uncovered_tcs == ["T1"]
    assert result.proposal.gap.reason.strip(), "gap.reason must be non-empty"
    reason_lower = result.proposal.gap.reason.lower()
    assert "http-mutating" in reason_lower or "allowed_ops" in reason_lower or "http-readonly" in reason_lower, (
        "gap.reason must mention the ops mismatch"
    )
    assert result.proposal.match is None
    assert result.proposal.new is None  # the worker did NOT invent a fake New
    assert result.proposal.bundle_hash == bundle.bundle_hash


def test_tc_077_cli_exits_zero_with_gap(monkeypatch, tmp_path) -> None:
    """CLI path: Gap is a valid outcome — worker still exits 0."""
    from verify_graph_author import __main__ as cli
    from verify_graph_author import worker as worker_mod
    from verify_graph_author.output import GraphProposal

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_gap_ops_mismatch()
    bundle_path = tmp_path / "bundle.json"
    bundle_path.write_text(bundle.model_dump_json(), encoding="utf-8")

    caller = make_caller(
        {
            "kind": "gap",
            "bundle_hash": bundle.bundle_hash,
            "gap": {
                "uncovered_tcs": ["T1"],
                "reason": (
                    "TC requires http-mutating but target environment allows "
                    "only http-readonly; cannot honestly cover with available vocabulary."
                ),
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
    assert code == 0, f"Gap must exit 0; stderr={stderr.getvalue()}"

    proposal = GraphProposal.model_validate_json(stdout.getvalue().strip())
    assert proposal.kind == "gap"
    assert proposal.gap is not None
    assert proposal.gap.uncovered_tcs == ["T1"]
    assert "http" in proposal.gap.reason.lower()


def test_tc_077_worker_rejects_invented_step_with_unsupported_op(monkeypatch) -> None:
    """If the LLM tries to fake a New with steps that need ops outside allowed_ops,
    the worker validates and re-prompts; on persistent failure it must NOT succeed
    (no garbage New gets through).
    """
    from verify_graph_author.worker import WorkerError, run_author

    assert_no_anthropic_attempt(monkeypatch)

    bundle = build_bundle_for_gap_ops_mismatch()

    # First response: tries to fake a New using http-request (needs http-mutating,
    # which is NOT in allowed_ops). Second response: same garbage again.
    fake_new = {
        "kind": "new",
        "bundle_hash": bundle.bundle_hash,
        "new": {
            "environment": "ENV-prod",
            "steps": [
                {
                    "step_type": "http-request",
                    "fields": {"method": "POST", "url": "/widgets"},
                    "provides_evidence_for": ["T1"],
                }
            ],
            "rationale": "Use http-request to POST a widget (this should not be allowed).",
        },
    }
    caller = make_caller(fake_new, fake_new)

    raised = False
    try:
        run_author(bundle, caller=caller)
    except WorkerError:
        raised = True
    assert raised, "worker must reject a New whose steps require ops outside allowed_ops"
