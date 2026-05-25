"""Thin LiteLLM client wrapper translating capability tags to model groups."""

from __future__ import annotations

import os
import time
from collections.abc import Awaitable, Callable
from dataclasses import dataclass, field
from typing import Any

# Default LiteLLM proxy endpoint per ADR-053 (slice-1 local-host LiteLLM).
DEFAULT_LITELLM_BASE_URL = "http://localhost:4000"

# Type alias for the underlying LiteLLM acompletion-style coroutine.
# Tests inject a fake of this signature so the SDK exercises its own
# wrapper logic without a running proxy.
CompletionFn = Callable[..., Awaitable[Any]]


@dataclass(frozen=True)
class LiteLLMConfig:
    """Endpoint configuration read from env per ADR-053.

    ``LITELLM_BASE_URL`` and ``LITELLM_API_KEY`` are injected by
    `pipeline-cli workers run`; nothing else in the SDK reads them.
    Moving the LiteLLM deployment is an env-var change, never a code
    change (ADR-053 §Consequences).
    """

    base_url: str = DEFAULT_LITELLM_BASE_URL
    api_key: str = ""
    timeout: float = 60.0

    @classmethod
    def from_env(cls, env: dict[str, str] | None = None) -> LiteLLMConfig:
        src = env if env is not None else os.environ
        return cls(
            base_url=src.get("LITELLM_BASE_URL", DEFAULT_LITELLM_BASE_URL),
            api_key=src.get("LITELLM_API_KEY", ""),
            timeout=float(src.get("LITELLM_TIMEOUT", "60")),
        )


@dataclass
class CompletionTelemetry:
    """Synchronous worker-side telemetry block per FT-081 §Telemetry.

    Authoritative for provenance (which model the proxy actually chose,
    how many retries it took, total wall latency). Cost is reconciled
    against LiteLLM's async callback POST later — see ADR-054 §"competing
    source-of-truth concern".
    """

    capability_tag: str
    model: str = ""
    provider: str = ""
    input_tokens: int = 0
    output_tokens: int = 0
    total_tokens: int = 0
    latency_seconds: float = 0.0
    retry_count: int = 0
    extra: dict[str, Any] = field(default_factory=dict)

    def merge_into(self, target: dict[str, Any]) -> dict[str, Any]:
        """Project the telemetry block onto a session telemetry dict.

        Counters are added (so multiple LLM calls in one session
        accumulate); scalar fields (model, provider) overwrite with the
        most-recent value.
        """
        target["capability_tag"] = self.capability_tag
        target["model"] = self.model
        target["provider"] = self.provider
        target["input_tokens"] = target.get("input_tokens", 0) + self.input_tokens
        target["output_tokens"] = target.get("output_tokens", 0) + self.output_tokens
        target["total_tokens"] = target.get("total_tokens", 0) + self.total_tokens
        target["latency_seconds"] = (
            target.get("latency_seconds", 0.0) + self.latency_seconds
        )
        target["retry_count"] = target.get("retry_count", 0) + self.retry_count
        for k, v in self.extra.items():
            target[k] = v
        return target


@dataclass(frozen=True)
class LiteLLMResult:
    """Output of a raw LiteLLM call before structured-output coercion."""

    content: str
    raw: Any
    telemetry: CompletionTelemetry


def _resolve_completion_fn(injected: CompletionFn | None) -> CompletionFn:
    """Return the injected fn, or import and bind ``litellm.acompletion``."""
    if injected is not None:
        return injected
    # Lazy import: ADR-054 mandates LiteLLM as the substrate, but tests
    # exercise this module without a running proxy by injecting a fake.
    # Pushing the import here keeps test environments lightweight.
    from litellm import acompletion  # type: ignore[import-untyped]

    return acompletion


def _extract_text_content(message: Any) -> str:
    """Pull the assistant text content off a LiteLLM/OpenAI-shaped message."""
    if message is None:
        return ""
    if isinstance(message, str):
        return message
    content = getattr(message, "content", None)
    if content is None and isinstance(message, dict):
        content = message.get("content", "")
    return content or ""


def _extract_telemetry(
    response: Any,
    *,
    capability_tag: str,
    latency_seconds: float,
    retry_count: int,
) -> CompletionTelemetry:
    """Pluck token counts + chosen provider/model out of a LiteLLM response."""
    usage = getattr(response, "usage", None)
    if usage is None and isinstance(response, dict):
        usage = response.get("usage")

    def _u(field_name: str) -> int:
        if usage is None:
            return 0
        if isinstance(usage, dict):
            return int(usage.get(field_name, 0) or 0)
        return int(getattr(usage, field_name, 0) or 0)

    model = getattr(response, "model", "")
    if not model and isinstance(response, dict):
        model = response.get("model", "")

    provider = ""
    hidden = getattr(response, "_hidden_params", None)
    if hidden is None and isinstance(response, dict):
        hidden = response.get("_hidden_params")
    if isinstance(hidden, dict):
        provider = hidden.get("custom_llm_provider", "") or ""

    return CompletionTelemetry(
        capability_tag=capability_tag,
        model=str(model or ""),
        provider=str(provider or ""),
        input_tokens=_u("prompt_tokens"),
        output_tokens=_u("completion_tokens"),
        total_tokens=_u("total_tokens"),
        latency_seconds=latency_seconds,
        retry_count=retry_count,
    )


class LiteLLMClient:
    """Thin async wrapper around ``litellm.acompletion`` for the SDK.

    The wrapper's only job is to:

    1. Read endpoint config from env (ADR-053).
    2. Pass the capability tag straight through as the LiteLLM ``model``
       parameter — LiteLLM's proxy config maps the tag to a model group
       (ADR-047 / ADR-054).
    3. Thread the DDD session ID into LiteLLM's ``metadata`` block so the
       async callback can correlate cost back to the originating session.
    4. Capture synchronous telemetry (tokens, latency, model chosen).

    Workers never see model names; they never see provider names. The
    indirection is enforced by the wrapper accepting only
    ``capability_tag`` as the model selector.
    """

    def __init__(
        self,
        config: LiteLLMConfig | None = None,
        *,
        completion_fn: CompletionFn | None = None,
        clock: Callable[[], float] = time.monotonic,
    ) -> None:
        self.config = config or LiteLLMConfig.from_env()
        self._completion_fn = completion_fn
        self._clock = clock

    def _completion(self) -> CompletionFn:
        # Resolved at call time so a test that monkeypatches
        # ``litellm.acompletion`` after construction still sees the swap.
        return _resolve_completion_fn(self._completion_fn)

    def _build_kwargs(
        self,
        *,
        capability_tag: str,
        messages: list[dict[str, Any]],
        metadata: dict[str, Any] | None,
        extra_body: dict[str, Any] | None,
        timeout: float | None,
        passthrough: dict[str, Any],
    ) -> dict[str, Any]:
        kwargs: dict[str, Any] = {
            "model": capability_tag,
            "messages": messages,
            "api_base": self.config.base_url,
            "timeout": timeout if timeout is not None else self.config.timeout,
        }
        if self.config.api_key:
            kwargs["api_key"] = self.config.api_key
        if metadata:
            kwargs["metadata"] = dict(metadata)
        if extra_body:
            kwargs["extra_body"] = dict(extra_body)
        for k, v in passthrough.items():
            if k not in kwargs:
                kwargs[k] = v
        return kwargs

    async def complete(
        self,
        *,
        capability_tag: str,
        messages: list[dict[str, Any]],
        metadata: dict[str, Any] | None = None,
        extra_body: dict[str, Any] | None = None,
        timeout: float | None = None,
        **passthrough: Any,
    ) -> LiteLLMResult:
        """Issue one LiteLLM call against the configured proxy.

        ``capability_tag`` is sent as the LiteLLM ``model`` parameter; the
        proxy resolves it to a model group via its own config. Any
        provider-specific request fields ride in ``extra_body``.
        ``metadata`` is propagated to LiteLLM unchanged so its logging
        callbacks can correlate the call back to a DDD session.
        """
        kwargs = self._build_kwargs(
            capability_tag=capability_tag,
            messages=messages,
            metadata=metadata,
            extra_body=extra_body,
            timeout=timeout,
            passthrough=passthrough,
        )
        completion = self._completion()
        start = self._clock()
        response = await completion(**kwargs)
        latency = max(0.0, self._clock() - start)
        choices = getattr(response, "choices", None) or (
            response.get("choices") if isinstance(response, dict) else None
        )
        first_choice = choices[0] if choices else None
        message = (
            getattr(first_choice, "message", None)
            if first_choice is not None
            else None
        )
        if message is None and isinstance(first_choice, dict):
            message = first_choice.get("message")
        content = _extract_text_content(message)
        retry_count = 0
        hidden = getattr(response, "_hidden_params", None)
        if hidden is None and isinstance(response, dict):
            hidden = response.get("_hidden_params")
        if isinstance(hidden, dict):
            retry_count = int(hidden.get("num_retries", 0) or 0)
        telemetry = _extract_telemetry(
            response,
            capability_tag=capability_tag,
            latency_seconds=latency,
            retry_count=retry_count,
        )
        return LiteLLMResult(content=content, raw=response, telemetry=telemetry)


__all__ = [
    "DEFAULT_LITELLM_BASE_URL",
    "CompletionFn",
    "CompletionTelemetry",
    "LiteLLMClient",
    "LiteLLMConfig",
    "LiteLLMResult",
]
