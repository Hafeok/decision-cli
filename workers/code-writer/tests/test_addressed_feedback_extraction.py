"""Tests for FT-108 addressed feedback extraction (FT-123)."""

from __future__ import annotations

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from code_writer.agent import run_agent
from code_writer.models import DefectFeedbackRecord, DispatchPayload


@pytest.fixture
def workspace(tmp_path: Path) -> Path:
    """Provide a temporary workspace."""
    ws = tmp_path / "workspace"
    ws.mkdir()
    return ws


@pytest.fixture
def payload_with_feedback(workspace: Path) -> DispatchPayload:
    """Provide a dispatch payload with defect feedback."""
    return DispatchPayload(
        dispatch_id="dispatch:001",
        session_id="session:001",
        feature_id="FT-123",
        bundle_markdown="# Test Bundle\n\nFix the defects.",
        bundle_hash="abc123",
        workspace_path=str(workspace),
        model_id="test-model",
        endpoint="anthropic",
        max_turns=4,
        allowed_tools=["read_file", "write_file"],
        defect_feedback=[
            DefectFeedbackRecord(
                feedback_iri="urn:dec:feedback:fb_001",
                severity="error",
                evidence="test failed",
                source_tc="https://decision-cli.dev/ns/tc/TC-001",
                graph_id="VG-001",
            ),
            DefectFeedbackRecord(
                feedback_iri="urn:dec:feedback:fb_002",
                severity="warning",
                evidence="gap in spec",
                source_tc="https://decision-cli.dev/ns/tc/TC-002",
                graph_id="VG-001",
            ),
        ],
    )


def test_extracts_citations_from_litellm_response(payload_with_feedback: DispatchPayload):
    """TC-281: FT-108 addressed feedback extraction still works post-migration."""
    # Mock response with citation block
    final_text = """Addressed urn:dec:feedback:fb_001 by updating the import.
urn:dec:feedback:fb_002 is out of scope for this dispatch.

<<DEC_ADDRESSED_FEEDBACK>>
{
  "iris": ["urn:dec:feedback:fb_001"]
}
<<END_DEC_ADDRESSED_FEEDBACK>>
"""

    mock_response = MagicMock()
    mock_response.choices = [MagicMock()]
    mock_response.choices[0].message = MagicMock()
    mock_response.choices[0].message.content = final_text
    mock_response.choices[0].message.tool_calls = None
    mock_response.choices[0].finish_reason = "stop"

    with patch("code_writer.agent.loop.litellm") as mock_litellm:
        mock_litellm.completion.return_value = mock_response

        with patch.dict("os.environ", {"LITELLM_API_KEY": "test-key"}):
            response = run_agent(payload_with_feedback)

    assert response.status == "ok"

    # Check that addressed_feedback was extracted correctly
    addressed = response.code_change.addressed_feedback_iris
    assert addressed == ["urn:dec:feedback:fb_001"]

    # fb_002 should NOT be in addressed (mention != address)
    assert "urn:dec:feedback:fb_002" not in addressed
