"""TC-105: Scaleway client wrapper builds with SCW_SECRET_KEY (FT-059).

CI mode (default): uses a fake ``OpenAI`` injected via monkeypatching.
Live mode (skipped unless ``SCALEWAY_LIVE=1`` and ``SCW_SECRET_KEY``):
issues a real chat-completions call against ``qwen3-coder-30b-a3b-instruct``.
"""

from __future__ import annotations

import logging
import os
import sys
from pathlib import Path
from types import SimpleNamespace
from typing import Any

import pytest

# Make the shared package importable without an editable install (mirrors
# the convention in test_emit_feedback.py).
HERE = Path(__file__).resolve()
SRC = HERE.parent.parent / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))

from _shared import scaleway_client  # noqa: E402
from _shared.scaleway_client import (  # noqa: E402
    SCALEWAY_BASE_URL,
    SCALEWAY_KEY_ENV,
    ScalewayClientError,
    build_client,
    missing_key_error_or_none,
    scaleway_chat_caller,
)


# ---------------------------------------------------------------------------
# Fakes
# ---------------------------------------------------------------------------


class _RecordingFakeOpenAI:
    """Stand-in for ``openai.OpenAI`` used in CI mode.

    Captures construction kwargs on instance attributes and records every
    ``chat.completions.create`` call so tests can assert on the request
    shape without a network call.
    """

    last_init_kwargs: dict[str, Any] = {}

    def __init__(self, **kwargs: Any) -> None:
        self.base_url = kwargs.get("base_url")
        self.api_key = kwargs.get("api_key")
        _RecordingFakeOpenAI.last_init_kwargs = dict(kwargs)
        self._response_factory: Any = None
        self.calls: list[dict[str, Any]] = []
        # Bridge to .chat.completions.create
        outer = self

        class _Completions:
            def create(self_inner, **call_kwargs: Any) -> Any:  # noqa: D401
                outer.calls.append(call_kwargs)
                if outer._response_factory is None:
                    return _default_fake_response("ok", 0, 0)
                return outer._response_factory(call_kwargs)

        self.chat = SimpleNamespace(completions=_Completions())

    # Convenience helpers used by tests.
    def set_response(self, factory: Any) -> None:
        self._response_factory = factory


def _default_fake_response(content: str, prompt_tokens: int, completion_tokens: int) -> Any:
    message = SimpleNamespace(content=content)
    choice = SimpleNamespace(message=message, finish_reason="stop")
    usage = SimpleNamespace(
        prompt_tokens=prompt_tokens,
        completion_tokens=completion_tokens,
    )
    return SimpleNamespace(choices=[choice], usage=usage)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture(autouse=True)
def _inject_fake_openai(monkeypatch: pytest.MonkeyPatch) -> None:
    """Replace the module-level ``OpenAI`` with the recording fake."""
    monkeypatch.setattr(scaleway_client, "OpenAI", _RecordingFakeOpenAI, raising=False)


@pytest.fixture
def _clear_key(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv(SCALEWAY_KEY_ENV, raising=False)


@pytest.fixture
def _set_test_key(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv(SCALEWAY_KEY_ENV, "test-key")


# ---------------------------------------------------------------------------
# Acceptance #1 — build_client constructs with key
# ---------------------------------------------------------------------------


def test_build_client_constructs_against_scaleway_with_key(_set_test_key: None) -> None:
    client = build_client()
    assert isinstance(client, _RecordingFakeOpenAI)
    assert client.base_url == SCALEWAY_BASE_URL
    assert client.base_url == "https://api.scaleway.ai/v1"
    assert client.api_key == "test-key"


# ---------------------------------------------------------------------------
# Acceptance #2 — build_client raises on missing key
# ---------------------------------------------------------------------------


def test_build_client_raises_scaleway_client_error_when_key_missing(
    _clear_key: None,
) -> None:
    with pytest.raises(ScalewayClientError) as exc_info:
        build_client()
    message = str(exc_info.value)
    assert SCALEWAY_KEY_ENV in message
    assert "endpoint=anthropic" in message
    assert exc_info.value.category == "missing_key"


# ---------------------------------------------------------------------------
# Acceptance #3 — missing_key_error_or_none is non-raising
# ---------------------------------------------------------------------------


def test_missing_key_error_or_none_returns_error_when_missing(_clear_key: None) -> None:
    result = missing_key_error_or_none()
    assert isinstance(result, ScalewayClientError)
    assert SCALEWAY_KEY_ENV in str(result)


def test_missing_key_error_or_none_returns_none_when_key_present(
    _set_test_key: None,
) -> None:
    assert missing_key_error_or_none() is None


def test_missing_key_error_or_none_treats_blank_value_as_missing(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv(SCALEWAY_KEY_ENV, "   ")
    result = missing_key_error_or_none()
    assert isinstance(result, ScalewayClientError)


# ---------------------------------------------------------------------------
# Acceptance #4 — scaleway_chat_caller request shape
# ---------------------------------------------------------------------------


def test_scaleway_chat_caller_passes_expected_kwargs(_set_test_key: None) -> None:
    client = build_client()
    caller = scaleway_chat_caller(client)
    client.set_response(
        lambda _kwargs: _default_fake_response("hello", 5, 7),
    )

    caller("sys", "user", "qwen3-coder-30b-a3b-instruct", 128)

    assert len(client.calls) == 1
    call = client.calls[0]
    assert call["model"] == "qwen3-coder-30b-a3b-instruct"
    assert call["max_tokens"] == 128
    assert call["messages"] == [
        {"role": "system", "content": "sys"},
        {"role": "user", "content": "user"},
    ]
    # reasoning_effort is not added unless explicitly requested.
    assert "reasoning_effort" not in call
    # extra_body must not be used as a wrapper for reasoning_effort.
    assert "extra_body" not in call


def test_scaleway_chat_caller_forwards_reasoning_effort_top_level(
    _set_test_key: None,
) -> None:
    client = build_client()
    caller = scaleway_chat_caller(client, reasoning_effort="medium")
    client.set_response(lambda _k: _default_fake_response("ok", 1, 1))

    caller("sys", "user", "gpt-oss-120b", 64)

    call = client.calls[0]
    assert call["reasoning_effort"] == "medium"
    assert "extra_body" not in call


def test_scaleway_chat_caller_uses_custom_temperature(_set_test_key: None) -> None:
    client = build_client()
    caller = scaleway_chat_caller(client, temperature=0.4)
    client.set_response(lambda _k: _default_fake_response("ok", 1, 1))
    caller("s", "u", "m", 32)
    assert client.calls[0]["temperature"] == pytest.approx(0.4)


# ---------------------------------------------------------------------------
# Acceptance #5 — token extraction
# ---------------------------------------------------------------------------


def test_scaleway_chat_caller_returns_text_and_token_counts(_set_test_key: None) -> None:
    client = build_client()
    caller = scaleway_chat_caller(client)
    client.set_response(lambda _k: _default_fake_response("the-content", 10, 20))

    text, tokens_in, tokens_out = caller("sys", "user", "qwen3-coder-30b-a3b-instruct", 128)

    assert text == "the-content"
    assert tokens_in == 10
    assert tokens_out == 20


def test_scaleway_chat_caller_handles_missing_usage(_set_test_key: None) -> None:
    client = build_client()
    caller = scaleway_chat_caller(client)

    def _no_usage(_kwargs: dict[str, Any]) -> Any:
        message = SimpleNamespace(content="text-only")
        choice = SimpleNamespace(message=message)
        return SimpleNamespace(choices=[choice], usage=None)

    client.set_response(_no_usage)

    text, tokens_in, tokens_out = caller("s", "u", "m", 32)
    assert text == "text-only"
    assert tokens_in == 0
    assert tokens_out == 0


# ---------------------------------------------------------------------------
# Acceptance #6 — no key logging
# ---------------------------------------------------------------------------


def test_wrapper_does_not_log_the_secret_key(
    _set_test_key: None, caplog: pytest.LogCaptureFixture
) -> None:
    caplog.set_level(logging.DEBUG)
    client = build_client()
    caller = scaleway_chat_caller(client)
    client.set_response(lambda _k: _default_fake_response("ok", 1, 1))
    caller("sys", "user", "m", 16)
    for record in caplog.records:
        assert "test-key" not in record.getMessage()
        # also defensive: not in args
        for arg in record.args or []:
            assert "test-key" not in str(arg)


# ---------------------------------------------------------------------------
# Acceptance #7 — live smoke (opt-in)
# ---------------------------------------------------------------------------


@pytest.mark.skipif(
    os.environ.get("SCALEWAY_LIVE", "").strip() != "1"
    or not os.environ.get("SCW_SECRET_KEY", "").strip(),
    reason="SCALEWAY_LIVE=1 and SCW_SECRET_KEY required for live smoke",
)
def test_scaleway_chat_caller_live_smoke(monkeypatch: pytest.MonkeyPatch) -> None:
    # Undo the autouse fake so the real OpenAI class is used.
    monkeypatch.undo()
    # Re-import to get the real OpenAI binding restored.
    import importlib
    module = importlib.reload(scaleway_client)
    monkeypatch.setenv(SCALEWAY_KEY_ENV, os.environ["SCW_SECRET_KEY"])

    client = module.build_client()
    caller = module.scaleway_chat_caller(client)
    text, tokens_in, tokens_out = caller(
        "reply with the single word OK",
        "hi",
        "qwen3-coder-30b-a3b-instruct",
        10,
    )
    assert "ok" in text.lower()
    assert tokens_in > 0
    assert tokens_out > 0
