"""Endpoint-agnostic ModelRouter dispatching to Scaleway or Anthropic (FT-060)."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal, Protocol

from ._endpoint_errors import (
    ErrorCategory,
    ModelRouterError,
    classify_endpoint_error as _classify_endpoint_error,
    scaleway_error_to_router_error as _scaleway_error_to_router_error,
)
from .scaleway_client import (
    _extract_message_content,
    _extract_message_reasoning,
    _extract_token_counts,
    build_client as build_scaleway_client,
    ScalewayClientError,
)
from .tools import translate_tools_for_anthropic

Endpoint = Literal["scaleway", "anthropic"]
ReasoningEffort = Literal["none", "low", "medium", "high"]

#: Tool name used to emulate structured output across endpoints. Both
#: routers declare a single tool with this name whose schema is the
#: caller-supplied ``response_schema``, and force ``tool_choice`` to it.
#: Anthropic uses ``input_schema``; Scaleway/OpenAI uses ``parameters``.
ANTHROPIC_STRUCTURED_OUTPUT_TOOL = "submit_verdict"


@dataclass
class ToolCall:
    """One tool invocation emitted by the model."""

    name: str
    arguments: dict[str, Any]
    call_id: str = ""


@dataclass
class CallParams:
    """Endpoint-agnostic request envelope routed by :class:`ModelRouter`."""

    endpoint: Endpoint
    model_identifier: str
    max_tokens: int
    temperature: float = 0.0
    reasoning_effort: ReasoningEffort | None = None
    tools: list[dict[str, Any]] | None = None
    response_schema: dict[str, Any] | None = None
    exposes_reasoning_trace: bool = False
    extra: dict[str, Any] = field(default_factory=dict)


@dataclass
class ModelResponse:
    """Normalised response shape across endpoints."""

    text: str
    tool_calls: list[ToolCall]
    tokens_in: int
    tokens_out: int
    tokens_cache_write: int = 0
    tokens_cache_hit: int = 0
    stop_reason: str = ""
    rationale_trace: str | None = None


class ModelRouter(Protocol):
    """Endpoint-agnostic caller contract."""

    def call(self, system: str, user: str, params: CallParams) -> ModelResponse: ...


# ---------------------------------------------------------------------------
# Scaleway router
# ---------------------------------------------------------------------------


@dataclass
class ScalewayRouter:
    """Scaleway OpenAI-compatible router (FT-059 client underneath)."""

    client: Any = None

    def _get_client(self) -> Any:
        if self.client is not None:
            return self.client
        try:
            return build_scaleway_client()
        except ScalewayClientError as exc:
            raise _scaleway_error_to_router_error(exc) from exc

    def call(self, system: str, user: str, params: CallParams) -> ModelResponse:
        client = self._get_client()
        kwargs = _build_scaleway_kwargs(system=system, user=user, params=params)
        try:
            response = client.chat.completions.create(**kwargs)
        except Exception as exc:  # noqa: BLE001 — normalised below
            raise _classify_endpoint_error(exc) from exc
        return _scaleway_response_to_model_response(response, params)


def _build_scaleway_kwargs(
    *, system: str, user: str, params: CallParams
) -> dict[str, Any]:
    """Build kwargs for ``chat.completions.create`` on a Scaleway endpoint.

    When ``response_schema`` is set the router mirrors the Anthropic path:
    it declares a single function tool whose ``parameters`` field is the
    JSON schema and forces ``tool_choice`` to that tool. Scaleway's
    OpenAI-compatible ``response_format={"type":"json_schema",...}`` was
    found to enforce only the top-level shape — nested ``required`` keys
    in objects-of-objects were ignored by Qwen3-Coder, so the worker had
    no way to extract a populated ``fields`` payload. Forcing a tool call
    eliminates the ambiguity: the model has to emit a function call
    matching the schema, and the SDK surfaces parsed arguments via
    ``message.tool_calls`` for downstream extraction.
    """
    kwargs: dict[str, Any] = {
        "model": params.model_identifier,
        "max_tokens": params.max_tokens,
        "temperature": params.temperature,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
    }
    if params.reasoning_effort is not None:
        kwargs["reasoning_effort"] = params.reasoning_effort

    tools: list[dict[str, Any]] = []
    if params.tools:
        tools.extend(params.tools)
    if params.response_schema is not None:
        structured_tool = {
            "type": "function",
            "function": {
                "name": ANTHROPIC_STRUCTURED_OUTPUT_TOOL,
                "description": "Emit the structured artifact required by the worker.",
                "parameters": params.response_schema,
            },
        }
        tools.append(structured_tool)
        kwargs["tool_choice"] = {
            "type": "function",
            "function": {"name": ANTHROPIC_STRUCTURED_OUTPUT_TOOL},
        }
    if tools:
        kwargs["tools"] = tools
    if params.extra:
        kwargs.update(params.extra)
    return kwargs


def _scaleway_response_to_model_response(
    response: Any, params: CallParams
) -> ModelResponse:
    """Lift a Scaleway response into the normalised :class:`ModelResponse`."""
    text = _extract_message_content(response)
    tokens_in, tokens_out = _extract_token_counts(response)
    tool_calls = _extract_scaleway_tool_calls(response)
    rationale_trace: str | None = None
    if params.exposes_reasoning_trace:
        rationale_trace = _extract_message_reasoning(response)
    stop_reason = _extract_scaleway_stop_reason(response)
    return ModelResponse(
        text=text,
        tool_calls=tool_calls,
        tokens_in=tokens_in,
        tokens_out=tokens_out,
        tokens_cache_write=0,
        tokens_cache_hit=0,
        stop_reason=stop_reason,
        rationale_trace=rationale_trace,
    )


def _extract_scaleway_tool_calls(response: Any) -> list[ToolCall]:
    """Pull ``message.tool_calls`` from a Scaleway response."""
    choices = getattr(response, "choices", None) or []
    if not choices:
        return []
    message = getattr(choices[0], "message", None)
    if message is None:
        return []
    raw_calls = getattr(message, "tool_calls", None) or []
    return [_scaleway_tool_call_to_tool_call(c) for c in raw_calls]


def _scaleway_tool_call_to_tool_call(raw: Any) -> ToolCall:
    """Normalise a Scaleway tool-call object/dict into a :class:`ToolCall`."""
    if isinstance(raw, dict):
        function = raw.get("function", {}) or {}
        name = function.get("name", "") if isinstance(function, dict) else ""
        args_text = function.get("arguments", "") if isinstance(function, dict) else ""
        call_id = raw.get("id", "") or ""
    else:
        function = getattr(raw, "function", None)
        name = getattr(function, "name", "") if function is not None else ""
        args_text = getattr(function, "arguments", "") if function is not None else ""
        call_id = getattr(raw, "id", "") or ""
    return ToolCall(name=name, arguments=_loads_arguments(args_text), call_id=call_id)


def _loads_arguments(args_text: Any) -> dict[str, Any]:
    """Parse a Scaleway tool-call arguments payload to a dict."""
    import json as _json

    if isinstance(args_text, dict):
        return args_text
    if not args_text:
        return {}
    try:
        parsed = _json.loads(args_text)
    except (ValueError, TypeError):
        return {}
    return parsed if isinstance(parsed, dict) else {}


def _extract_scaleway_stop_reason(response: Any) -> str:
    choices = getattr(response, "choices", None) or []
    if not choices:
        return ""
    return getattr(choices[0], "finish_reason", "") or ""


# ---------------------------------------------------------------------------
# Anthropic router
# ---------------------------------------------------------------------------


@dataclass
class AnthropicRouter:
    """Anthropic SDK router (FT-060 §Outputs)."""

    client: Any = None

    def _get_client(self) -> Any:
        if self.client is not None:
            return self.client
        try:
            import anthropic  # type: ignore[import-not-found]
        except ImportError as exc:  # pragma: no cover
            raise ModelRouterError(
                "anthropic SDK is not installed", category="unknown"
            ) from exc
        return anthropic.Anthropic()

    def call(self, system: str, user: str, params: CallParams) -> ModelResponse:
        client = self._get_client()
        kwargs = _build_anthropic_kwargs(system=system, user=user, params=params)
        try:
            response = client.messages.create(**kwargs)
        except Exception as exc:  # noqa: BLE001 — normalised below
            raise _classify_endpoint_error(exc) from exc
        return _anthropic_response_to_model_response(response)


def _build_anthropic_kwargs(
    *, system: str, user: str, params: CallParams
) -> dict[str, Any]:
    """Build kwargs for ``messages.create`` on an Anthropic endpoint."""
    kwargs: dict[str, Any] = {
        "model": params.model_identifier,
        "max_tokens": params.max_tokens,
        "system": system,
        "messages": [{"role": "user", "content": user}],
    }
    # ``temperature`` honoured when explicitly set (default 0.0 is valid).
    kwargs["temperature"] = params.temperature

    tools: list[dict[str, Any]] = []
    if params.tools:
        tools.extend(translate_tools_for_anthropic(params.tools))
    if params.response_schema is not None:
        structured_tool = {
            "name": ANTHROPIC_STRUCTURED_OUTPUT_TOOL,
            "description": "Emit the structured artifact required by the worker.",
            "input_schema": params.response_schema,
        }
        tools.append(structured_tool)
        kwargs["tool_choice"] = {
            "type": "tool",
            "name": ANTHROPIC_STRUCTURED_OUTPUT_TOOL,
        }
    if tools:
        kwargs["tools"] = tools
    # ``reasoning_effort`` is silently ignored on Anthropic — extended
    # thinking is a separate parameter not exposed in this slice (PRD
    # §10.6, FT-060 boundaries).
    return kwargs


def _anthropic_response_to_model_response(response: Any) -> ModelResponse:
    """Lift an Anthropic ``Message`` into the normalised :class:`ModelResponse`."""
    text, tool_calls = _split_anthropic_content(response)
    usage = getattr(response, "usage", None)
    tokens_in = int(getattr(usage, "input_tokens", 0) or 0) if usage else 0
    tokens_out = int(getattr(usage, "output_tokens", 0) or 0) if usage else 0
    cache_write = int(getattr(usage, "cache_creation_input_tokens", 0) or 0) if usage else 0
    cache_hit = int(getattr(usage, "cache_read_input_tokens", 0) or 0) if usage else 0
    stop_reason = getattr(response, "stop_reason", "") or ""
    return ModelResponse(
        text=text,
        tool_calls=tool_calls,
        tokens_in=tokens_in,
        tokens_out=tokens_out,
        tokens_cache_write=cache_write,
        tokens_cache_hit=cache_hit,
        stop_reason=stop_reason,
        rationale_trace=None,
    )


def _split_anthropic_content(response: Any) -> tuple[str, list[ToolCall]]:
    """Separate text blocks from ``tool_use`` blocks in an Anthropic response."""
    content = getattr(response, "content", None) or []
    text_parts: list[str] = []
    tool_calls: list[ToolCall] = []
    for block in content:
        block_type = _block_field(block, "type")
        if block_type == "text":
            text = _block_field(block, "text") or ""
            if text:
                text_parts.append(text)
        elif block_type == "tool_use":
            tool_calls.append(_anthropic_block_to_tool_call(block))
    return "\n".join(text_parts).strip(), tool_calls


def _block_field(block: Any, name: str) -> Any:
    """Read a field from an Anthropic content block (dict or object)."""
    if isinstance(block, dict):
        return block.get(name)
    return getattr(block, name, None)


def _anthropic_block_to_tool_call(block: Any) -> ToolCall:
    """Normalise an Anthropic ``tool_use`` block into a :class:`ToolCall`."""
    name = _block_field(block, "name") or ""
    inputs = _block_field(block, "input") or {}
    call_id = _block_field(block, "id") or ""
    if not isinstance(inputs, dict):
        inputs = {}
    return ToolCall(name=name, arguments=inputs, call_id=call_id)


# ---------------------------------------------------------------------------
# Router factory + error classification
# ---------------------------------------------------------------------------


def build_router(endpoint: Endpoint | str, *, client: Any = None) -> ModelRouter:
    """Construct a :class:`ModelRouter` for ``endpoint``.

    Raises ``ModelRouterError(category='unknown')`` for unrecognised
    endpoints — the dispatcher is responsible for validating the
    capability's endpoint at resolution time (FT-061); this is a defence
    in depth at the worker boundary.
    """
    if endpoint == "scaleway":
        return ScalewayRouter(client=client)
    if endpoint == "anthropic":
        return AnthropicRouter(client=client)
    raise ModelRouterError(
        f"unknown endpoint: {endpoint!r}", category="unknown"
    )


