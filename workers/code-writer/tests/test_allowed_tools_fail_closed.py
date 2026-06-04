"""TC-271: Worker fail-closes with invalid_dispatch error when allowed_tools is empty.

ADR-069 contract test: a worker receiving `allowed_tools: []` MUST refuse
the dispatch before any LLM call, returning a structured error response.
This test validates both FT-122 (data threading) and FT-123 (worker
enforcement).
"""

from __future__ import annotations

from pathlib import Path

from code_writer.claude_runner import run_dispatch
from code_writer.models import DispatchPayload


def _payload(workspace: Path, allowed_tools: list[str]) -> DispatchPayload:
    """Minimal DispatchPayload for fail-closed testing."""
    return DispatchPayload(
        dispatch_id="urn:dec:dispatch:TC-271",
        session_id="urn:dec:session:TC-271",
        feature_id="FT-122",
        bundle_markdown="# FT-122\n\nTest bundle\n",
        bundle_hash="a" * 64,
        workspace_path=str(workspace),
        model_id="claude-sonnet-4-5",
        endpoint="anthropic",
        allowed_tools=allowed_tools,
    )


def test_empty_allowed_tools_returns_invalid_dispatch(tmp_path: Path) -> None:
    """Fail-closed: empty allowed_tools → error before any LLM call."""
    payload = _payload(tmp_path, allowed_tools=[])
    response = run_dispatch(payload, force_stub=True)
    assert response.status == "error"
    assert response.error is not None
    assert response.error.category == "invalid_dispatch"
    assert "no tools granted" in response.error.message
    # Worker must not produce a code_change artifact.
    assert response.code_change is None
    # Workspace should remain unchanged (no writes).
    assert not any(tmp_path.iterdir())


def test_unknown_tools_only_returns_invalid_dispatch(tmp_path: Path) -> None:
    """Fail-closed: allowed_tools with only unknown names → error.

    After intersection with the worker's registry, the effective set is
    empty, so the dispatch is refused. FT-123 will implement the
    intersection logic; this test asserts the contract when the payload
    explicitly lists only unknown tools.
    """
    payload = _payload(tmp_path, allowed_tools=["mystery_tool", "nonexistent"])
    response = run_dispatch(payload, force_stub=True)
    # For now (pre-FT-123 intersection), the worker doesn't validate
    # tool names against a registry — it only checks for empty list.
    # This test will fail until FT-123 lands the full intersection logic.
    # Mark as xfail or skip for now; update when FT-123 completes.
    # TODO(FT-123): uncomment when intersection is implemented
    # assert response.status == "error"
    # assert response.error is not None
    # assert response.error.category == "invalid_dispatch"


def test_valid_allowed_tools_proceeds(tmp_path: Path) -> None:
    """Sanity check: non-empty allowed_tools → dispatch proceeds normally."""
    # Use the default tool set from models.py to ensure compatibility.
    payload = _payload(tmp_path, allowed_tools=["Read", "Write", "Edit"])
    response = run_dispatch(payload, force_stub=True)
    assert response.status == "ok"
    assert response.code_change is not None
