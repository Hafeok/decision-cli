"""Pydantic structured-output layer on top of LiteLLMClient via instructor."""

from __future__ import annotations

import json
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Any, TypeVar

from pydantic import BaseModel, ValidationError

from .litellm_client import (
    CompletionTelemetry,
    LiteLLMClient,
    LiteLLMConfig,
)

T = TypeVar("T", bound=BaseModel)

# Type alias for an injectable structured-output coercer. Real deployments
# wire instructor's ``from_litellm(...)`` here; tests inject a stub that
# returns a known Pydantic instance so the SDK exercises its own logic
# without a running proxy.
StructuredFn = Callable[..., Awaitable[Any]]


@dataclass(frozen=True)
class CompletionResult:
    """The output of ``Provider.complete()``.

    ``output`` is the Pydantic instance the caller asked for via
    ``output_schema=``. ``telemetry`` is the synchronous worker-side
    telemetry block that gets merged into the session's completion
    payload — authoritative for provenance per FT-081.
    """

    output: BaseModel
    telemetry: CompletionTelemetry
    raw_content: str = ""


def _coerce_from_text(content: str, schema: type[T]) -> T:
    """Fallback coercion when no instructor client is available.

    Attempts a plain ``model_validate_json`` first, then a relaxed
    ``model_validate`` on a parsed dict. Raises ``ValidationError`` if
    the assistant text cannot be coerced — that's the structured-output
    contract: the worker either gets a valid instance back or sees a
    parseable exception.
    """
    text = (content or "").strip()
    try:
        return schema.model_validate_json(text)
    except ValidationError:
        pass
    try:
        return schema.model_validate(json.loads(text))
    except (json.JSONDecodeError, ValidationError) as exc:
        raise ValidationError.from_exception_data(
            title=schema.__name__,
            line_errors=[],
        ) from exc


class Provider:
    """Capability-tag aware structured-output provider for the worker SDK.

    Sits on top of ``LiteLLMClient`` and exposes one call surface:

        result = await provider.complete(
            capability_tag="frontier-reasoning",
            messages=[...],
            output_schema=SomePydanticModel,
            metadata={"ddd_session_id": session.id},
        )

    Resolution rules:

    - ``capability_tag`` is sent straight through as LiteLLM's ``model``
      parameter; the proxy maps the tag to a configured model group per
      ADR-047/ADR-054. Workers never see model names.
    - ``output_schema`` is a Pydantic model. Structured output is enforced
      via instructor when an instructor client is wired in (production
      path) or via best-effort JSON coercion when no instructor client is
      configured (test/CI path) — both paths return a validated Pydantic
      instance or raise.
    - ``metadata`` is forwarded to LiteLLM untouched. The DDD session ID
      rides in ``metadata["ddd_session_id"]`` so LiteLLM's async logging
      callback can correlate cost telemetry back to the session record on
      the harness side.
    - Provider-specific request fields ride in ``extra_body`` (Anthropic
      tool use, OpenAI ``response_format``, etc.) per ADR-054. The SDK
      does not add per-provider methods.
    """

    def __init__(
        self,
        client: LiteLLMClient | None = None,
        *,
        config: LiteLLMConfig | None = None,
        structured_fn: StructuredFn | None = None,
    ) -> None:
        self._client = client or LiteLLMClient(config=config)
        self._structured_fn = structured_fn

    @property
    def client(self) -> LiteLLMClient:
        return self._client

    @property
    def config(self) -> LiteLLMConfig:
        return self._client.config

    async def complete(
        self,
        *,
        capability_tag: str,
        messages: list[dict[str, Any]],
        output_schema: type[T],
        metadata: dict[str, Any] | None = None,
        extra_body: dict[str, Any] | None = None,
        timeout: float | None = None,
        **passthrough: Any,
    ) -> CompletionResult:
        """Dispatch one capability-tag call and return a typed result.

        Returns a :class:`CompletionResult` whose ``output`` is an
        instance of ``output_schema`` and whose ``telemetry`` is the
        synchronous worker-side telemetry block ready to be merged into
        the session.
        """
        if self._structured_fn is not None:
            return await self._complete_via_structured(
                capability_tag=capability_tag,
                messages=messages,
                output_schema=output_schema,
                metadata=metadata,
                extra_body=extra_body,
                timeout=timeout,
                passthrough=passthrough,
            )
        return await self._complete_via_text(
            capability_tag=capability_tag,
            messages=messages,
            output_schema=output_schema,
            metadata=metadata,
            extra_body=extra_body,
            timeout=timeout,
            passthrough=passthrough,
        )

    async def _complete_via_text(
        self,
        *,
        capability_tag: str,
        messages: list[dict[str, Any]],
        output_schema: type[T],
        metadata: dict[str, Any] | None,
        extra_body: dict[str, Any] | None,
        timeout: float | None,
        passthrough: dict[str, Any],
    ) -> CompletionResult:
        """Path used when no instructor client is wired (test/CI)."""
        result = await self._client.complete(
            capability_tag=capability_tag,
            messages=messages,
            metadata=metadata,
            extra_body=extra_body,
            timeout=timeout,
            **passthrough,
        )
        output = _coerce_from_text(result.content, output_schema)
        return CompletionResult(
            output=output,
            telemetry=result.telemetry,
            raw_content=result.content,
        )

    async def _complete_via_structured(
        self,
        *,
        capability_tag: str,
        messages: list[dict[str, Any]],
        output_schema: type[T],
        metadata: dict[str, Any] | None,
        extra_body: dict[str, Any] | None,
        timeout: float | None,
        passthrough: dict[str, Any],
    ) -> CompletionResult:
        """Path used when an instructor client is wired (production).

        The injected ``structured_fn`` must accept the same kwargs
        ``LiteLLMClient.complete`` would receive plus ``response_model=``
        and must return either the Pydantic instance directly or a
        tuple of (instance, raw_response). When the second form is
        returned the SDK uses the raw_response to extract telemetry;
        otherwise telemetry is captured from the underlying LiteLLM
        call signature without token counts.
        """
        kwargs: dict[str, Any] = {
            "model": capability_tag,
            "messages": messages,
            "response_model": output_schema,
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

        import time

        start = time.monotonic()
        response = await self._structured_fn(**kwargs)
        latency = max(0.0, time.monotonic() - start)

        if isinstance(response, tuple) and len(response) == 2:
            instance, raw = response
            telemetry = _telemetry_from_raw(
                raw,
                capability_tag=capability_tag,
                latency=latency,
            )
            raw_content = _content_from_raw(raw)
        else:
            instance = response
            telemetry = CompletionTelemetry(
                capability_tag=capability_tag,
                latency_seconds=latency,
            )
            raw_content = ""

        if not isinstance(instance, output_schema):
            raise TypeError(
                f"structured_fn returned {type(instance).__name__}, "
                f"expected {output_schema.__name__}"
            )
        return CompletionResult(
            output=instance, telemetry=telemetry, raw_content=raw_content
        )


def _telemetry_from_raw(
    raw: Any, *, capability_tag: str, latency: float
) -> CompletionTelemetry:
    """Pluck token counts off a LiteLLM raw response for the structured path."""
    usage = getattr(raw, "usage", None)
    if usage is None and isinstance(raw, dict):
        usage = raw.get("usage")

    def _u(field_name: str) -> int:
        if usage is None:
            return 0
        if isinstance(usage, dict):
            return int(usage.get(field_name, 0) or 0)
        return int(getattr(usage, field_name, 0) or 0)

    model = getattr(raw, "model", "") or (
        raw.get("model", "") if isinstance(raw, dict) else ""
    )
    hidden = getattr(raw, "_hidden_params", None)
    if hidden is None and isinstance(raw, dict):
        hidden = raw.get("_hidden_params")
    provider = ""
    retry_count = 0
    if isinstance(hidden, dict):
        provider = hidden.get("custom_llm_provider", "") or ""
        retry_count = int(hidden.get("num_retries", 0) or 0)
    return CompletionTelemetry(
        capability_tag=capability_tag,
        model=str(model or ""),
        provider=str(provider or ""),
        input_tokens=_u("prompt_tokens"),
        output_tokens=_u("completion_tokens"),
        total_tokens=_u("total_tokens"),
        latency_seconds=latency,
        retry_count=retry_count,
    )


def _content_from_raw(raw: Any) -> str:
    choices = getattr(raw, "choices", None) or (
        raw.get("choices") if isinstance(raw, dict) else None
    )
    if not choices:
        return ""
    first = choices[0]
    message = getattr(first, "message", None) or (
        first.get("message") if isinstance(first, dict) else None
    )
    if message is None:
        return ""
    content = getattr(message, "content", None) or (
        message.get("content", "") if isinstance(message, dict) else ""
    )
    return content or ""


__all__ = [
    "CompletionResult",
    "Provider",
    "StructuredFn",
]
