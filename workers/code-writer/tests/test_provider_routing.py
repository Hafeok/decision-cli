"""Tests for provider routing via LiteLLM (FT-123)."""

from __future__ import annotations

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
        bundle_markdown="# Test Bundle",
        bundle_hash="abc123",
        workspace_path=str(workspace),
        model_id="test-model",
        endpoint="anthropic",
        max_turns=4,
        allowed_tools=["read_file", "write_file"],
    )


def test_anthropic_routes_via_litellm(payload: DispatchPayload):
    """TC-278: Anthropic endpoint routes via LiteLLM with model group resolution."""
    payload.endpoint = "anthropic"
    payload.model_id = "anthropic-claude-sonnet-4-5"

    # Mock response
    mock_response = MagicMock()
    mock_response.choices = [MagicMock()]
    mock_response.choices[0].message = MagicMock()
    mock_response.choices[0].message.content = "Done."
    mock_response.choices[0].message.tool_calls = None
    mock_response.choices[0].finish_reason = "stop"

    with patch("code_writer.agent.loop.litellm") as mock_litellm:
        mock_litellm.completion.return_value = mock_response

        with patch.dict(
            "os.environ",
            {"LITELLM_API_KEY": "sk-virtual-001", "LITELLM_BASE_URL": "http://proxy.test:4000"},
        ):
            response = run_agent(payload)

    assert response.status == "ok"

    # Verify LiteLLM was called with correct parameters
    call_kwargs = mock_litellm.completion.call_args.kwargs
    assert call_kwargs["model"] == "anthropic-claude-sonnet-4-5"
    assert call_kwargs["api_key"] == "sk-virtual-001"
    assert call_kwargs["base_url"] == "http://proxy.test:4000"


def test_scaleway_routes_via_litellm(payload: DispatchPayload):
    """TC-279: Scaleway endpoint routes via LiteLLM with model group resolution."""
    payload.endpoint = "scaleway"
    payload.model_id = "scaleway-llama-3.3-70b-instruct"

    # Mock response
    mock_response = MagicMock()
    mock_response.choices = [MagicMock()]
    mock_response.choices[0].message = MagicMock()
    mock_response.choices[0].message.content = "Done."
    mock_response.choices[0].message.tool_calls = None
    mock_response.choices[0].finish_reason = "stop"

    with patch("code_writer.agent.loop.litellm") as mock_litellm:
        mock_litellm.completion.return_value = mock_response

        # Ensure NO SCW_SECRET_KEY or DEC_YROUTER_URL in env
        env = {"LITELLM_API_KEY": "sk-virtual-001", "LITELLM_BASE_URL": "http://proxy.test:4000"}
        # Explicitly remove any Scaleway-specific vars
        with patch.dict("os.environ", env, clear=False):
            # Remove SCW vars if they exist
            import os

            for key in ["SCW_SECRET_KEY", "DEC_YROUTER_URL"]:
                os.environ.pop(key, None)

            response = run_agent(payload)

    assert response.status == "ok"

    # Verify LiteLLM was called with correct parameters (same shape as Anthropic)
    call_kwargs = mock_litellm.completion.call_args.kwargs
    assert call_kwargs["model"] == "scaleway-llama-3.3-70b-instruct"
    assert call_kwargs["api_key"] == "sk-virtual-001"
    assert call_kwargs["base_url"] == "http://proxy.test:4000"


def test_missing_litellm_key_fails_closed(payload: DispatchPayload):
    """TC-280: missing LITELLM_API_KEY returns structured invalid_dispatch error."""
    # Case A: LITELLM_API_KEY unset
    mock_response = MagicMock()
    mock_response.choices = [MagicMock()]

    with patch("code_writer.agent.loop.litellm") as mock_litellm:
        mock_litellm.completion.side_effect = AssertionError("should not be called")

        with patch.dict("os.environ", {"LITELLM_BASE_URL": "http://proxy.test:4000"}, clear=True):
            response = run_agent(payload)

    assert response.status == "error"
    assert response.error.category == "invalid_dispatch"
    assert "LITELLM_API_KEY" in response.error.message
    mock_litellm.completion.assert_not_called()

    # Case B: LITELLM_API_KEY empty string
    with patch("code_writer.agent.loop.litellm") as mock_litellm:
        mock_litellm.completion.side_effect = AssertionError("should not be called")

        with patch.dict("os.environ", {"LITELLM_API_KEY": "", "LITELLM_BASE_URL": "http://proxy.test:4000"}):
            response = run_agent(payload)

    assert response.status == "error"
    assert response.error.category == "invalid_dispatch"
    assert "LITELLM_API_KEY" in response.error.message
    mock_litellm.completion.assert_not_called()

    # Case C: LITELLM_BASE_URL absent but key present (should succeed with default)
    mock_response = MagicMock()
    mock_response.choices = [MagicMock()]
    mock_response.choices[0].message = MagicMock()
    mock_response.choices[0].message.content = "Done."
    mock_response.choices[0].message.tool_calls = None
    mock_response.choices[0].finish_reason = "stop"

    with patch("code_writer.agent.loop.litellm") as mock_litellm:
        mock_litellm.completion.return_value = mock_response

        with patch.dict("os.environ", {"LITELLM_API_KEY": "sk-virtual-001"}, clear=True):
            response = run_agent(payload)

    assert response.status == "ok"
    # Should use default base_url
    call_kwargs = mock_litellm.completion.call_args.kwargs
    assert call_kwargs["base_url"] == "http://localhost:4000"
