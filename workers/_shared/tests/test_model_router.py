"""TC-106: ModelRouter routes Scaleway and Anthropic uniformly (FT-060).

Tests the endpoint-agnostic ``ModelRouter`` abstraction in
``workers/_shared/src/_shared/model_router.py``: routing, tool
translation, structured outputs across endpoints, reasoning_effort
passthrough, error normalisation, and the verifier worker's
construction of ``CallParams`` from the bundle.

All tests use stub endpoints — no live calls.
"""

from __future__ import annotations

import sys
from pathlib import Path
from types import SimpleNamespace
from typing import Any

import pytest

# Make the shared package importable without an editable install.
HERE = Path(__file__).resolve()
SRC = HERE.parent.parent / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))

from _shared.model_router import (  # noqa: E402
    ANTHROPIC_STRUCTURED_OUTPUT_TOOL,
    AnthropicRouter,
    CallParams,
    ModelResponse,
    ModelRouterError,
    ScalewayRouter,
    ToolCall,
    build_router,
)
from _shared.tools import (  # noqa: E402
    IMPLEMENTER_TOOLS,
    openai_tool_to_anthropic,
    translate_tools_for_anthropic,
)


# ---------------------------------------------------------------------------
# Fakes
# ---------------------------------------------------------------------------


class _FakeScalewayClient:
    """Records ``chat.completions.create`` invocations."""

    def __init__(self) -> None:
        self.calls: list[dict[str, Any]] = []
        self._response: Any = _scaleway_response(content="ok", tokens=(1, 1))
        outer = self

        class _Completions:
            def create(self_inner, **kwargs: Any) -> Any:  # noqa: D401
                outer.calls.append(kwargs)
                response = outer._response
                if isinstance(response, Exception):
                    raise response
                if callable(response):
                    return response(kwargs)
                return response

        self.chat = SimpleNamespace(completions=_Completions())

    def set_response(self, response: Any) -> None:
        self._response = response


class _FakeAnthropicClient:
    """Records ``messages.create`` invocations."""

    def __init__(self) -> None:
        self.calls: list[dict[str, Any]] = []
        self._response: Any = _anthropic_response(text="ok")
        outer = self

        class _Messages:
            def create(self_inner, **kwargs: Any) -> Any:  # noqa: D401
                outer.calls.append(kwargs)
                response = outer._response
                if isinstance(response, Exception):
                    raise response
                if callable(response):
                    return response(kwargs)
                return response

        self.messages = _Messages()

    def set_response(self, response: Any) -> None:
        self._response = response


def _scaleway_response(
    *,
    content: str = "",
    tokens: tuple[int, int] = (0, 0),
    tool_calls: list[Any] | None = None,
    reasoning: str | None = None,
    finish_reason: str = "stop",
) -> Any:
    message = SimpleNamespace(
        content=content,
        tool_calls=tool_calls or [],
        reasoning=reasoning,
    )
    choice = SimpleNamespace(message=message, finish_reason=finish_reason)
    usage = SimpleNamespace(prompt_tokens=tokens[0], completion_tokens=tokens[1])
    return SimpleNamespace(choices=[choice], usage=usage)


def _anthropic_response(
    *,
    text: str = "",
    tool_uses: list[dict[str, Any]] | None = None,
    tokens: tuple[int, int] = (0, 0),
    cache_write: int = 0,
    cache_hit: int = 0,
    stop_reason: str = "end_turn",
) -> Any:
    content: list[Any] = []
    if text:
        content.append(SimpleNamespace(type="text", text=text))
    for tu in tool_uses or []:
        content.append(
            SimpleNamespace(
                type="tool_use",
                name=tu.get("name", ""),
                input=tu.get("input", {}),
                id=tu.get("id", "tool_call_1"),
            )
        )
    usage = SimpleNamespace(
        input_tokens=tokens[0],
        output_tokens=tokens[1],
        cache_creation_input_tokens=cache_write,
        cache_read_input_tokens=cache_hit,
    )
    return SimpleNamespace(content=content, usage=usage, stop_reason=stop_reason)


def _basic_params(endpoint: str, **overrides: Any) -> CallParams:
    base = {
        "endpoint": endpoint,
        "model_identifier": "test-model-x",
        "max_tokens": 256,
        "temperature": 0.0,
    }
    base.update(overrides)
    return CallParams(**base)  # type: ignore[arg-type]


# ---------------------------------------------------------------------------
# Acceptance #1 — endpoint routing
# ---------------------------------------------------------------------------


def test_build_router_returns_scaleway_router() -> None:
    router = build_router("scaleway")
    assert isinstance(router, ScalewayRouter)


def test_build_router_returns_anthropic_router() -> None:
    router = build_router("anthropic")
    assert isinstance(router, AnthropicRouter)


def test_build_router_raises_unknown_for_invalid_endpoint() -> None:
    with pytest.raises(ModelRouterError) as exc_info:
        build_router("invalid")  # type: ignore[arg-type]
    assert exc_info.value.category == "unknown"


# ---------------------------------------------------------------------------
# Acceptance #2 — uniform response shape
# ---------------------------------------------------------------------------


def test_scaleway_call_returns_uniform_model_response() -> None:
    client = _FakeScalewayClient()
    client.set_response(
        _scaleway_response(
            content="hello world",
            tokens=(11, 22),
            tool_calls=[
                {
                    "id": "call_x",
                    "function": {"name": "read_file", "arguments": '{"path": "foo.rs"}'},
                }
            ],
        )
    )
    router = ScalewayRouter(client=client)
    response = router.call("sys", "user", _basic_params("scaleway"))
    _assert_response_shape(response)
    assert response.text == "hello world"
    assert response.tokens_in == 11
    assert response.tokens_out == 22
    assert len(response.tool_calls) == 1
    assert response.tool_calls[0].name == "read_file"
    assert response.tool_calls[0].arguments == {"path": "foo.rs"}


def test_anthropic_call_returns_uniform_model_response() -> None:
    client = _FakeAnthropicClient()
    client.set_response(
        _anthropic_response(
            text="hello world",
            tool_uses=[{"name": "read_file", "input": {"path": "foo.rs"}}],
            tokens=(11, 22),
        )
    )
    router = AnthropicRouter(client=client)
    response = router.call("sys", "user", _basic_params("anthropic"))
    _assert_response_shape(response)
    assert response.text == "hello world"
    assert response.tokens_in == 11
    assert response.tokens_out == 22
    assert len(response.tool_calls) == 1
    assert response.tool_calls[0].name == "read_file"
    assert response.tool_calls[0].arguments == {"path": "foo.rs"}


def _assert_response_shape(response: Any) -> None:
    assert isinstance(response, ModelResponse)
    assert isinstance(response.text, str)
    assert isinstance(response.tool_calls, list)
    for tc in response.tool_calls:
        assert isinstance(tc, ToolCall)
    assert isinstance(response.tokens_in, int)
    assert isinstance(response.tokens_out, int)
    assert isinstance(response.stop_reason, str)


# ---------------------------------------------------------------------------
# Acceptance #3 — tool schema translation
# ---------------------------------------------------------------------------


_EDIT_FILE_TOOL = {
    "type": "function",
    "function": {
        "name": "edit_file",
        "description": "Edit a file in the workspace.",
        "parameters": {
            "type": "object",
            "properties": {"path": {"type": "string"}, "content": {"type": "string"}},
            "required": ["path", "content"],
        },
    },
}


def test_anthropic_router_translates_openai_tools_to_tool_use_shape() -> None:
    client = _FakeAnthropicClient()
    router = AnthropicRouter(client=client)
    params = _basic_params("anthropic", tools=[_EDIT_FILE_TOOL])
    router.call("sys", "user", params)
    sent_tools = client.calls[0]["tools"]
    assert sent_tools == [
        {
            "name": "edit_file",
            "description": "Edit a file in the workspace.",
            "input_schema": _EDIT_FILE_TOOL["function"]["parameters"],
        }
    ]


def test_scaleway_router_passes_openai_tools_unchanged() -> None:
    client = _FakeScalewayClient()
    router = ScalewayRouter(client=client)
    params = _basic_params("scaleway", tools=[_EDIT_FILE_TOOL])
    router.call("sys", "user", params)
    sent_tools = client.calls[0]["tools"]
    assert sent_tools == [_EDIT_FILE_TOOL]


def test_openai_tool_to_anthropic_helper_matches_invariant() -> None:
    translated = openai_tool_to_anthropic(_EDIT_FILE_TOOL)
    assert translated["input_schema"] == _EDIT_FILE_TOOL["function"]["parameters"]
    assert translated["name"] == _EDIT_FILE_TOOL["function"]["name"]


def test_implementer_tool_catalog_translates_cleanly() -> None:
    translated = translate_tools_for_anthropic(IMPLEMENTER_TOOLS)
    assert len(translated) == len(IMPLEMENTER_TOOLS)
    for src, dst in zip(IMPLEMENTER_TOOLS, translated):
        assert dst["name"] == src["function"]["name"]
        assert dst["input_schema"] == src["function"]["parameters"]


# ---------------------------------------------------------------------------
# Acceptance #4 — structured output on Scaleway
# ---------------------------------------------------------------------------


_SAMPLE_SCHEMA = {
    "type": "object",
    "properties": {
        "verdict": {"type": "string"},
        "rationale": {"type": "string"},
    },
    "required": ["verdict", "rationale"],
}


def test_scaleway_router_translates_response_schema_to_forced_tool_call() -> None:
    """Scaleway/OpenAI-compatible structured output is emitted via a forced
    function-call tool whose ``parameters`` carry the JSON schema.

    Previously the router used ``response_format={"type":"json_schema",...}``,
    but Qwen3-Coder on Scaleway honoured only the top-level shape — nested
    ``required`` keys (e.g. ``step.fields.command``) were silently dropped,
    producing empty payloads. Forcing a tool call closes that loophole and
    mirrors the Anthropic structured-output path.
    """
    client = _FakeScalewayClient()
    router = ScalewayRouter(client=client)
    params = _basic_params("scaleway", response_schema=_SAMPLE_SCHEMA)
    router.call("sys", "user", params)
    call_kwargs = client.calls[0]
    # No longer using response_format — the structured output rides on tools.
    assert "response_format" not in call_kwargs
    sent_tools = call_kwargs["tools"]
    structured = [
        t for t in sent_tools
        if t.get("type") == "function"
        and t.get("function", {}).get("name") == ANTHROPIC_STRUCTURED_OUTPUT_TOOL
    ]
    assert len(structured) == 1
    assert structured[0]["function"]["parameters"] == _SAMPLE_SCHEMA
    assert call_kwargs["tool_choice"] == {
        "type": "function",
        "function": {"name": ANTHROPIC_STRUCTURED_OUTPUT_TOOL},
    }


# ---------------------------------------------------------------------------
# Acceptance #5 — structured output on Anthropic
# ---------------------------------------------------------------------------


def test_anthropic_router_emulates_structured_output_via_submit_verdict_tool() -> None:
    client = _FakeAnthropicClient()
    router = AnthropicRouter(client=client)
    params = _basic_params("anthropic", response_schema=_SAMPLE_SCHEMA)
    router.call("sys", "user", params)
    call_kwargs = client.calls[0]
    sent_tools = call_kwargs["tools"]
    structured = [t for t in sent_tools if t["name"] == ANTHROPIC_STRUCTURED_OUTPUT_TOOL]
    assert len(structured) == 1
    assert structured[0]["input_schema"] == _SAMPLE_SCHEMA
    assert call_kwargs["tool_choice"] == {
        "type": "tool",
        "name": ANTHROPIC_STRUCTURED_OUTPUT_TOOL,
    }


# ---------------------------------------------------------------------------
# Acceptance #6 — reasoning_effort passthrough
# ---------------------------------------------------------------------------


def test_scaleway_router_forwards_reasoning_effort_top_level() -> None:
    client = _FakeScalewayClient()
    router = ScalewayRouter(client=client)
    params = _basic_params("scaleway", reasoning_effort="medium")
    router.call("sys", "user", params)
    call = client.calls[0]
    assert call["reasoning_effort"] == "medium"
    assert "extra_body" not in call


def test_anthropic_router_ignores_reasoning_effort_silently() -> None:
    client = _FakeAnthropicClient()
    router = AnthropicRouter(client=client)
    params = _basic_params("anthropic", reasoning_effort="high")
    response = router.call("sys", "user", params)
    assert isinstance(response, ModelResponse)
    assert "reasoning_effort" not in client.calls[0]


# ---------------------------------------------------------------------------
# Acceptance #7 — error normalisation
# ---------------------------------------------------------------------------


class _AuthError(Exception):
    """Anthropic-shaped authentication failure."""


class _RateLimitError(Exception):
    """Anthropic-shaped rate-limit failure."""


class _NetworkError(Exception):
    """Generic network / timeout failure."""


class _InvalidError(Exception):
    """Generic invalid-response failure."""


@pytest.mark.parametrize(
    "exception,expected_category",
    [
        (_AuthError("401 Unauthorized: invalid API key"), "auth_failed"),
        (_RateLimitError("429 Too Many Requests"), "rate_limited"),
        (_NetworkError("connection timeout after 30s"), "network_error"),
        (_InvalidError("400 Bad Request: invalid schema field"), "invalid_response"),
    ],
)
def test_anthropic_errors_are_normalised_into_router_error(
    exception: Exception, expected_category: str
) -> None:
    client = _FakeAnthropicClient()
    client.set_response(exception)
    router = AnthropicRouter(client=client)
    with pytest.raises(ModelRouterError) as exc_info:
        router.call("sys", "user", _basic_params("anthropic"))
    assert exc_info.value.category == expected_category


def test_scaleway_errors_are_normalised_into_router_error() -> None:
    client = _FakeScalewayClient()
    client.set_response(_AuthError("401 invalid api key"))
    router = ScalewayRouter(client=client)
    with pytest.raises(ModelRouterError) as exc_info:
        router.call("sys", "user", _basic_params("scaleway"))
    assert exc_info.value.category == "auth_failed"


# ---------------------------------------------------------------------------
# Acceptance #8 — verifier worker uses router (constructs CallParams from bundle)
# ---------------------------------------------------------------------------


def test_verifier_worker_constructs_call_params_from_bundle() -> None:
    # Add the verifier package to sys.path so we can import without
    # requiring a uv-tool install in CI.
    repo_root = HERE.parent.parent.parent.parent
    verifier_src = repo_root / "workers" / "verifier" / "src"
    if str(verifier_src) not in sys.path:
        sys.path.insert(0, str(verifier_src))

    from verifier.bundle import VerifierInput  # noqa: PLC0415
    from verifier.worker import build_call_params  # noqa: PLC0415

    bundle = VerifierInput.model_validate(
        {
            "dispatch_id": "urn:dec:dispatch:1",
            "dispatch_group": "urn:dec:group:1",
            "interpretation_session": "urn:dec:session:interp-1",
            "action_session": "urn:dec:session:action-1",
            "feature_id": "FT-013",
            "feature_spec": "spec",
            "produced_artifact": "diff",
            "bundle_hash": "deadbeef" * 8,
            "in_stream": "https://decision-cli.dev/stream/test",
            "model_id": "qwen3-coder-30b-a3b-instruct",
            "max_tokens": 2048,
        }
    )

    params = build_call_params(
        bundle,
        endpoint="scaleway",
        reasoning_effort="medium",
    )

    assert isinstance(params, CallParams)
    assert params.endpoint == "scaleway"
    assert params.model_identifier == "qwen3-coder-30b-a3b-instruct"
    assert params.max_tokens == 2048
    assert params.reasoning_effort == "medium"
    # Schema is derived from the Pydantic model, not a hardcoded constant.
    assert params.response_schema is not None
    assert params.response_schema.get("type") == "object"
    assert "verdict" in params.response_schema.get("properties", {})


def test_verifier_run_via_router_drives_router_with_bundle_pinned_model() -> None:
    repo_root = HERE.parent.parent.parent.parent
    verifier_src = repo_root / "workers" / "verifier" / "src"
    if str(verifier_src) not in sys.path:
        sys.path.insert(0, str(verifier_src))

    from verifier.bundle import VerifierInput  # noqa: PLC0415
    from verifier.worker import run_verifier_via_router  # noqa: PLC0415

    bundle = VerifierInput.model_validate(
        {
            "dispatch_id": "urn:dec:dispatch:1",
            "dispatch_group": "urn:dec:group:1",
            "interpretation_session": "urn:dec:session:interp-1",
            "action_session": "urn:dec:session:action-1",
            "feature_id": "FT-013",
            "feature_spec": "spec",
            "produced_artifact": "diff",
            "bundle_hash": "deadbeef" * 8,
            "in_stream": "https://decision-cli.dev/stream/test",
            "model_id": "qwen3-coder-30b-a3b-instruct",
            "max_tokens": 2048,
        }
    )

    seen_params: list[CallParams] = []
    canned_payload = (
        '{"verdict": "approved", '
        '"rationale": "stub router payload satisfies the verifier schema.", '
        '"violates": []}'
    )

    class _StubRouter:
        def call(self, system: str, user: str, params: CallParams) -> ModelResponse:
            seen_params.append(params)
            return ModelResponse(
                text=canned_payload,
                tool_calls=[],
                tokens_in=7,
                tokens_out=11,
                stop_reason="end_turn",
            )

    result = run_verifier_via_router(bundle, _StubRouter(), endpoint="scaleway")

    assert result.verdict.verdict == "approved"
    assert result.telemetry.attempts == 1
    assert result.telemetry.input_tokens == 7
    assert result.telemetry.output_tokens == 11
    assert len(seen_params) == 1
    assert seen_params[0].endpoint == "scaleway"
    assert seen_params[0].model_identifier == "qwen3-coder-30b-a3b-instruct"


# ---------------------------------------------------------------------------
# Bonus — reasoning trace ingestion (PRD §10.6)
# ---------------------------------------------------------------------------


def test_scaleway_router_surfaces_reasoning_trace_when_capability_exposes_one() -> None:
    client = _FakeScalewayClient()
    client.set_response(
        _scaleway_response(content="answer", reasoning="step 1 -> step 2 -> done")
    )
    router = ScalewayRouter(client=client)
    params = _basic_params("scaleway", exposes_reasoning_trace=True)
    response = router.call("sys", "user", params)
    assert response.rationale_trace == "step 1 -> step 2 -> done"


def test_scaleway_router_does_not_surface_trace_when_flag_is_false() -> None:
    client = _FakeScalewayClient()
    client.set_response(
        _scaleway_response(content="answer", reasoning="latent chain")
    )
    router = ScalewayRouter(client=client)
    response = router.call("sys", "user", _basic_params("scaleway"))
    assert response.rationale_trace is None


# ---------------------------------------------------------------------------
# Bonus — Anthropic cache token extraction
# ---------------------------------------------------------------------------


def test_anthropic_router_records_cache_token_counts() -> None:
    client = _FakeAnthropicClient()
    client.set_response(
        _anthropic_response(
            text="hi",
            tokens=(100, 50),
            cache_write=80,
            cache_hit=120,
        )
    )
    router = AnthropicRouter(client=client)
    response = router.call("sys", "user", _basic_params("anthropic"))
    assert response.tokens_cache_write == 80
    assert response.tokens_cache_hit == 120


def test_anthropic_router_defaults_cache_tokens_to_zero_when_absent() -> None:
    client = _FakeAnthropicClient()
    # Build a response whose usage block lacks the cache fields.
    usage = SimpleNamespace(input_tokens=10, output_tokens=5)
    client.set_response(
        SimpleNamespace(
            content=[SimpleNamespace(type="text", text="hi")],
            usage=usage,
            stop_reason="end_turn",
        )
    )
    router = AnthropicRouter(client=client)
    response = router.call("sys", "user", _basic_params("anthropic"))
    assert response.tokens_cache_write == 0
    assert response.tokens_cache_hit == 0
