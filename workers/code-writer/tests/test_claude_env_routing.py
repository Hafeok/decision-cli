"""Env-routing tests for FT-066 / ADR-033.

These tests cover the pure ``claude_env_for`` translator:

* ``test_scaleway_env``       — TC-115
* ``test_anthropic_env``      — TC-116
* ``test_missing_scw_key``    — TC-117

The runner is constructed deterministically: no subprocess calls, no
network, no workspace mutation. Each test patches the process env via
``monkeypatch`` so SCW_SECRET_KEY / DEC_YROUTER_URL behave predictably
under CI.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from code_writer._subprocess_runner import (
    _endpoint_config_response,
    _missing_binary_response,
)
from code_writer.env_routing import (
    DEFAULT_YROUTER_URL,
    EndpointConfigError,
    claude_env_for,
)
from code_writer.models import DispatchPayload, WorkerResponse


def _payload(
    workspace: Path,
    *,
    endpoint: str,
    model_id: str = "qwen3-coder-30b-a3b-instruct",
) -> DispatchPayload:
    return DispatchPayload(
        dispatch_id="urn:dec:dispatch:FT-066-test",
        session_id="urn:dec:session:FT-066-test",
        feature_id="FT-066",
        bundle_markdown="# FT-066 routing test bundle\n",
        bundle_hash="b" * 64,
        workspace_path=str(workspace),
        model_id=model_id,
        endpoint=endpoint,  # type: ignore[arg-type]
    )


def test_scaleway_env(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """TC-115 — Scaleway routing injects all required env vars."""
    monkeypatch.setenv("SCW_SECRET_KEY", "scw-test-key")
    monkeypatch.delenv("DEC_YROUTER_URL", raising=False)

    payload = _payload(
        tmp_path, endpoint="scaleway", model_id="qwen3-coder-30b-a3b-instruct"
    )
    env = claude_env_for(payload)

    # y-router proxy base URL + Scaleway secret key as bearer token.
    assert env["ANTHROPIC_BASE_URL"] == DEFAULT_YROUTER_URL
    assert env["ANTHROPIC_BASE_URL"] == "http://localhost:8787"
    assert env["ANTHROPIC_AUTH_TOKEN"] == "scw-test-key"
    # All five Claude Code model-slot env vars pinned to the resolved model.
    expected_model = "qwen3-coder-30b-a3b-instruct"
    for var in (
        "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "ANTHROPIC_SMALL_FAST_MODEL",
        "ANTHROPIC_DEFAULT_SONET_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
    ):
        assert env[var] == expected_model, f"{var} not pinned to model_id"


def test_scaleway_env_respects_dec_yrouter_url_override(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A custom proxy address must propagate to ANTHROPIC_BASE_URL."""
    monkeypatch.setenv("SCW_SECRET_KEY", "scw-test-key")
    monkeypatch.setenv("DEC_YROUTER_URL", "http://proxy.local:9999/")

    payload = _payload(tmp_path, endpoint="scaleway")
    env = claude_env_for(payload)

    # Trailing slash should be stripped.
    assert env["ANTHROPIC_BASE_URL"] == "http://proxy.local:9999"


def test_anthropic_env(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """TC-116 — Anthropic routing pins ANTHROPIC_MODEL only."""
    # Even with a stray SCW key set, the anthropic branch must ignore it.
    monkeypatch.setenv("SCW_SECRET_KEY", "should-not-be-used")
    monkeypatch.delenv("ANTHROPIC_BASE_URL", raising=False)
    monkeypatch.delenv("ANTHROPIC_AUTH_TOKEN", raising=False)

    payload = _payload(tmp_path, endpoint="anthropic", model_id="claude-opus-4-7")
    env = claude_env_for(payload)

    assert env["ANTHROPIC_MODEL"] == "claude-opus-4-7"
    # Critical: must NOT override the base URL (Claude Code default path).
    assert "ANTHROPIC_BASE_URL" not in env
    # Auth token is also not set by the anthropic branch.
    assert "ANTHROPIC_AUTH_TOKEN" not in env
    # Slot-specific vars are NOT pinned on the anthropic branch — leaving
    # Claude Code's own tier resolution intact.
    for var in (
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "ANTHROPIC_SMALL_FAST_MODEL",
        "ANTHROPIC_DEFAULT_SONET_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
    ):
        assert var not in env, f"{var} unexpectedly pinned on anthropic branch"


def test_missing_scw_key(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """TC-117 — missing SCW_SECRET_KEY surfaces structured endpoint_config error."""
    monkeypatch.delenv("SCW_SECRET_KEY", raising=False)
    monkeypatch.delenv("DEC_YROUTER_URL", raising=False)

    payload = _payload(tmp_path, endpoint="scaleway")

    with pytest.raises(EndpointConfigError) as excinfo:
        claude_env_for(payload)

    assert excinfo.value.category == "missing_credentials"
    assert "SCW_SECRET_KEY" in str(excinfo.value)

    # Wire the error through the same mapping the runner uses pre-spawn
    # and assert the structured WorkerResponse shape: status="error",
    # error.category="endpoint_config", retryable=False.
    response = _endpoint_config_response(payload, excinfo.value)
    assert isinstance(response, WorkerResponse)
    assert response.status == "error"
    assert response.error is not None
    assert response.error.category == "endpoint_config"
    assert response.error.retryable is False


def test_missing_scw_key_empty_string_also_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Whitespace-only SCW_SECRET_KEY is treated as missing."""
    monkeypatch.setenv("SCW_SECRET_KEY", "   ")
    payload = _payload(tmp_path, endpoint="scaleway")
    with pytest.raises(EndpointConfigError) as excinfo:
        claude_env_for(payload)
    assert excinfo.value.category == "missing_credentials"


def test_unsupported_endpoint_raises_structured_error(tmp_path: Path) -> None:
    """An endpoint outside the literal whitelist surfaces endpoint_config."""
    # Construct the payload via model_validate so we can bypass the
    # Literal validation and exercise the runtime fallback branch.
    blob = {
        "dispatch_id": "urn:dec:dispatch:1",
        "session_id": "urn:dec:session:1",
        "feature_id": "FT-066",
        "bundle_markdown": "# bundle\n",
        "bundle_hash": "c" * 64,
        "workspace_path": str(tmp_path),
        "model_id": "claude-sonnet-4-5",
        "endpoint": "anthropic",
    }
    payload = DispatchPayload.model_validate(blob)
    # Force an unsupported value after construction.
    object.__setattr__(payload, "endpoint", "openai")
    with pytest.raises(EndpointConfigError) as excinfo:
        claude_env_for(payload)
    assert excinfo.value.category == "unsupported_endpoint"


def test_anthropic_branch_does_not_use_missing_binary(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Sanity: the missing-binary helper is unrelated to endpoint routing."""
    payload = _payload(tmp_path, endpoint="anthropic")
    # The helper returns a structured WorkerResponse the runner would
    # have used; assert it is well-formed and uses a different category.
    response = _missing_binary_response(payload)
    assert response.status == "error"
    assert response.error is not None
    assert response.error.category == "subscription_unavailable"
