"""Tests for the in-process agent loop (FT-123)."""

from __future__ import annotations

import subprocess
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from code_writer.agent import run_agent
from code_writer.models import DispatchPayload


@pytest.fixture
def workspace(tmp_path: Path) -> Path:
    """Provide a temporary workspace."""
    ws = tmp_path / "workspace"
    ws.mkdir()
    return ws


@pytest.fixture
def payload(workspace: Path) -> DispatchPayload:
    """Provide a basic dispatch payload."""
    return DispatchPayload(
        dispatch_id="dispatch:001",
        session_id="session:001",
        feature_id="FT-123",
        bundle_markdown="# Test Bundle\n\nImplement the feature.",
        bundle_hash="abc123",
        workspace_path=str(workspace),
        model_id="test-model",
        endpoint="anthropic",
        max_turns=4,
        timeout_seconds=300,
        allowed_tools=["read_file", "write_file"],
    )


def test_single_write_completes(payload: DispatchPayload, workspace: Path):
    """TC-275: single write_file tool call completes with status=ok and one FileWrite."""
    # Mock litellm.completion to return a write tool call, then end_turn
    mock_response_1 = MagicMock()
    mock_response_1.choices = [MagicMock()]
    mock_response_1.choices[0].message = MagicMock()
    mock_response_1.choices[0].message.content = ""
    mock_response_1.choices[0].finish_reason = "tool_calls"

    # Create tool call
    tool_call = MagicMock()
    tool_call.id = "call_1"
    tool_call.function = MagicMock()
    tool_call.function.name = "write_file"
    tool_call.function.arguments = '{"path": "hello.py", "content": "print(\'hi\')\\n", "old_string": null}'

    mock_response_1.choices[0].message.tool_calls = [tool_call]

    # Second response: end_turn
    mock_response_2 = MagicMock()
    mock_response_2.choices = [MagicMock()]
    mock_response_2.choices[0].message = MagicMock()
    mock_response_2.choices[0].message.content = "Created hello.py."
    mock_response_2.choices[0].message.tool_calls = None
    mock_response_2.choices[0].finish_reason = "stop"

    with patch("code_writer.agent.loop.litellm") as mock_litellm:
        mock_litellm.completion.side_effect = [mock_response_1, mock_response_2]

        with patch.dict("os.environ", {"LITELLM_API_KEY": "test-key"}):
            response = run_agent(payload)

    assert response.status == "ok"
    assert (workspace / "hello.py").exists()
    assert (workspace / "hello.py").read_text() == "print('hi')\n"
    assert len(response.code_change.files) == 1
    assert response.code_change.files[0].path == "hello.py"
    assert len(response.telemetry.tool_calls) == 1
    assert response.telemetry.tool_calls[0].name == "write_file"
    assert response.telemetry.tool_calls[0].result_status == "ok"
    assert mock_litellm.completion.call_count == 2


def test_max_turns_exceeded(payload: DispatchPayload, workspace: Path):
    """TC-276: honours max_turns and returns max_turns_exceeded with partial telemetry."""
    # Create a README file for the read tool to access
    (workspace / "README.md").write_text("# Test README")

    # Mock to always return read_file tool call
    def mock_completion_call(*args, **kwargs):
        mock_response = MagicMock()
        mock_response.choices = [MagicMock()]
        mock_response.choices[0].message = MagicMock()
        mock_response.choices[0].message.content = ""
        mock_response.choices[0].finish_reason = "tool_calls"

        tool_call = MagicMock()
        tool_call.id = f"call_{mock_completion_call.call_count}"
        tool_call.function = MagicMock()
        tool_call.function.name = "read_file"
        tool_call.function.arguments = '{"path": "README.md"}'

        mock_response.choices[0].message.tool_calls = [tool_call]
        mock_completion_call.call_count += 1
        return mock_response

    mock_completion_call.call_count = 0

    with patch("code_writer.agent.loop.litellm") as mock_litellm:
        mock_litellm.completion.side_effect = mock_completion_call

        with patch.dict("os.environ", {"LITELLM_API_KEY": "test-key"}):
            payload.max_turns = 2
            response = run_agent(payload)

    assert response.status == "error"
    assert response.error.category == "timeout"
    assert "max_turns" in response.error.message.lower()
    assert mock_litellm.completion.call_count == 2
    assert len(response.telemetry.tool_calls) == 2
    # Only read_file was called, so no workspace modifications
    assert not list(workspace.glob("*.py"))


def test_no_model_subprocess(payload: DispatchPayload):
    """TC-277: no model-invoking subprocess fires during a dispatch."""
    # Patch subprocess to raise if called
    original_popen = subprocess.Popen
    original_run = subprocess.run

    def forbidden_popen(*args, **kwargs):
        raise AssertionError("subprocess.Popen invoked during model-only dispatch")

    def forbidden_run(*args, **kwargs):
        raise AssertionError("subprocess.run invoked during model-only dispatch")

    # Mock litellm to return end_turn immediately
    mock_response = MagicMock()
    mock_response.choices = [MagicMock()]
    mock_response.choices[0].message = MagicMock()
    mock_response.choices[0].message.content = "Done."
    mock_response.choices[0].message.tool_calls = None
    mock_response.choices[0].finish_reason = "stop"

    with patch("subprocess.Popen", side_effect=forbidden_popen):
        with patch("subprocess.run", side_effect=forbidden_run):
            with patch("code_writer.agent.loop.litellm") as mock_litellm:
                mock_litellm.completion.return_value = mock_response

                with patch.dict("os.environ", {"LITELLM_API_KEY": "test-key"}):
                    response = run_agent(payload)

    assert response.status == "ok"


def test_unknown_tool_names_dropped(payload: DispatchPayload, caplog):
    """TC-282: unknown tool names dropped with a warn log."""
    # Add an unknown tool to allowed_tools
    payload.allowed_tools = ["read_file", "write_file", "apply_patch"]

    # Mock litellm
    mock_response = MagicMock()
    mock_response.choices = [MagicMock()]
    mock_response.choices[0].message = MagicMock()
    mock_response.choices[0].message.content = "Done."
    mock_response.choices[0].message.tool_calls = None
    mock_response.choices[0].finish_reason = "stop"

    with patch("code_writer.agent.loop.litellm") as mock_litellm:
        mock_litellm.completion.return_value = mock_response

        with patch.dict("os.environ", {"LITELLM_API_KEY": "test-key"}):
            response = run_agent(payload)

    assert response.status == "ok"

    # Check that tools argument only contains known tools
    call_args = mock_litellm.completion.call_args
    tools = call_args.kwargs.get("tools", [])
    tool_names = {t["function"]["name"] for t in tools}
    assert tool_names == {"read_file", "write_file"}
    assert "apply_patch" not in tool_names
