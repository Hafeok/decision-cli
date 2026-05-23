"""Scaleway OpenAI-compatible client wrapper for decision-cli workers (FT-059)."""

from __future__ import annotations

import os
from typing import Any, Callable

#: Scaleway's OpenAI-compatible serverless inference endpoint (PRD §14).
SCALEWAY_BASE_URL = "https://api.scaleway.ai/v1"

#: Environment variable holding the Scaleway secret key. Stored alongside
#: ``ANTHROPIC_API_KEY`` per [ADR-037](ADR-037).
SCALEWAY_KEY_ENV = "SCW_SECRET_KEY"

#: Hint included in the missing-key error to point operators at the
#: rebinding escape hatch when Scaleway is unavailable.
_MISSING_KEY_HINT = (
    "SCW_SECRET_KEY not set; install Scaleway key or rebind capability "
    "to endpoint=anthropic"
)

#: Bound categories surfaced via ``ScalewayClientError.category``. Used by
#: the dispatcher to decide retry / fail / escalate per [ADR-034](ADR-034).
ErrorCategory = str

# Callable shape matching the verifier worker's ``ModelCaller``
# ``(system, user, model_id, max_tokens) -> (raw_json, tokens_in, tokens_out)``.
ModelCaller = Callable[[str, str, str, int], tuple[str, int, int]]

# Lazy module-level binding for ``openai.OpenAI``. Imported at module load
# time when the SDK is available; tests inject a fake class via
# monkeypatching ``_shared.scaleway_client.OpenAI``.
try:  # pragma: no cover - exercised by both branches in CI
    from openai import OpenAI  # type: ignore[import-not-found]
except ImportError:  # pragma: no cover - exercised when SDK is absent
    OpenAI = None  # type: ignore[assignment]


class ScalewayClientError(Exception):
    """Raised when the Scaleway wrapper cannot proceed.

    The ``category`` attribute lets the dispatcher discriminate retryable
    rate-limits from terminal auth / config errors.
    """

    def __init__(self, message: str, *, category: ErrorCategory = "config") -> None:
        super().__init__(message)
        self.category = category


def missing_key_error_or_none() -> ScalewayClientError | None:
    """Return a structured error if ``SCW_SECRET_KEY`` is missing.

    Non-raising: returns ``None`` when the key is present so callers can
    surface "the worker cannot dispatch because Scaleway key is missing"
    as a session error rather than crashing the dispatcher.
    """
    if not os.environ.get(SCALEWAY_KEY_ENV, "").strip():
        return ScalewayClientError(_MISSING_KEY_HINT, category="missing_key")
    return None


def build_client() -> Any:
    """Construct an ``openai.OpenAI`` client pointed at Scaleway.

    Reads ``SCW_SECRET_KEY`` from the environment exactly once per call.
    The wrapper does not log or otherwise persist the key; it is passed
    straight to the SDK.

    Raises:
        ScalewayClientError: if the env var is missing or the SDK is not
            installed.
    """
    error = missing_key_error_or_none()
    if error is not None:
        raise error
    if OpenAI is None:
        raise ScalewayClientError(
            "openai SDK is not installed; install it with "
            "`pip install 'decision-cli-shared[scaleway]'`",
            category="config",
        )
    api_key = os.environ[SCALEWAY_KEY_ENV]
    return OpenAI(base_url=SCALEWAY_BASE_URL, api_key=api_key)


def _extract_message_content(response: Any) -> str:
    """Pull text content from an OpenAI chat-completions response."""
    choices = getattr(response, "choices", None) or []
    if not choices:
        return ""
    message = getattr(choices[0], "message", None)
    if message is None:
        return ""
    content = getattr(message, "content", None)
    return content if isinstance(content, str) else ""


def _extract_message_reasoning(response: Any) -> str | None:
    """Pull the optional reasoning trace from a response.

    Frontier-reasoning Scaleway capabilities expose a separate reasoning
    chain as ``message.reasoning``; capabilities that don't surface it
    simply produce ``None``. The capability's ``exposes_reasoning_trace``
    flag tells the router whether to read this field (PRD §14).
    """
    choices = getattr(response, "choices", None) or []
    if not choices:
        return None
    message = getattr(choices[0], "message", None)
    if message is None:
        return None
    return getattr(message, "reasoning", None)


def _extract_token_counts(response: Any) -> tuple[int, int]:
    """Return ``(prompt_tokens, completion_tokens)`` defaulting to 0."""
    usage = getattr(response, "usage", None)
    if usage is None:
        return 0, 0
    tokens_in = int(getattr(usage, "prompt_tokens", 0) or 0)
    tokens_out = int(getattr(usage, "completion_tokens", 0) or 0)
    return tokens_in, tokens_out


def _build_chat_kwargs(
    *,
    system: str,
    user: str,
    model_id: str,
    max_tokens: int,
    temperature: float,
    reasoning_effort: str | None,
) -> dict[str, Any]:
    """Assemble kwargs for ``chat.completions.create``.

    ``reasoning_effort`` is a top-level kwarg on the Scaleway API per
    PRD §14 (verified against a reasoning-capable Scaleway endpoint);
    do not wrap it inside ``extra_body``.
    """
    kwargs: dict[str, Any] = {
        "model": model_id,
        "max_tokens": max_tokens,
        "temperature": temperature,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
    }
    if reasoning_effort is not None:
        kwargs["reasoning_effort"] = reasoning_effort
    return kwargs


def scaleway_chat_caller(
    client: Any,
    *,
    temperature: float = 0.0,
    reasoning_effort: str | None = None,
) -> ModelCaller:
    """Return a ``ModelCaller`` closure that issues a Scaleway chat call.

    The closure matches the verifier worker's ``ModelCaller`` signature
    so the worker harness can dispatch to either Anthropic or Scaleway
    via the same indirection. Token counts come from
    ``response.usage.prompt_tokens`` / ``completion_tokens``;
    Scaleway does not currently support prompt caching so the cache
    write / hit fields are recorded as zero by [FT-057](FT-057).
    """

    def _call(system: str, user: str, model_id: str, max_tokens: int) -> tuple[str, int, int]:
        kwargs = _build_chat_kwargs(
            system=system,
            user=user,
            model_id=model_id,
            max_tokens=max_tokens,
            temperature=temperature,
            reasoning_effort=reasoning_effort,
        )
        response = client.chat.completions.create(**kwargs)
        text = _extract_message_content(response)
        tokens_in, tokens_out = _extract_token_counts(response)
        return text, tokens_in, tokens_out

    return _call


def extract_reasoning_trace(response: Any) -> str | None:
    """Public re-export of the reasoning-trace extractor.

    [FT-060](FT-060)'s ``ModelRouter`` consumes this to attach the
    ``rationale_trace`` evidence to the produced artifact when the
    resolved capability has ``exposes_reasoning_trace = true``.
    """
    return _extract_message_reasoning(response)
