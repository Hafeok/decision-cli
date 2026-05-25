"""TC-146: provider.complete capability-tag dispatch + structured output.

Exit criterion for FT-081 (pipeline-worker SDK LiteLLM client with
capability-tag dispatch and structured output).

The four facts the parent feature_spec names — that the test must prove:

1. A worker call to ``Provider.complete(capability_tag="frontier-reasoning",
   …)`` resolves the capability tag to the model group configured in
   LiteLLM and returns a Pydantic instance conforming to ``output_schema``.
2. Synchronous telemetry (tokens, latency, model, retry count, provider
   chosen) appears alongside the result so the session can attach it to
   the completion payload — authoritative for provenance per FT-081.
3. The DDD session id rides through to LiteLLM's metadata block so the
   async cost-telemetry callback can reconcile against the worker-reported
   telemetry on the harness side.
4. Moving the LiteLLM endpoint (changing ``LITELLM_BASE_URL``) requires
   no SDK or worker code changes — the SDK reads env at construction.

Because TC-146 must run without a live LiteLLM proxy or real provider
keys, we drive ``LiteLLMClient`` with an injected fake ``acompletion``
function and ``Provider`` with an injected ``structured_fn``. The
fakes return OpenAI/LiteLLM-shaped objects; the SDK code paths under
test are the production paths — only the network call at the bottom is
stubbed.
"""

from __future__ import annotations

from typing import Any

import pytest
from pydantic import BaseModel

from pipeline_worker_sdk import (
    DEFAULT_LITELLM_BASE_URL,
    CompletionResult,
    CompletionTelemetry,
    LiteLLMClient,
    LiteLLMConfig,
    Provider,
    Session,
)
from pipeline_worker_sdk.types import DispatchEvent


# --------------------------------------------------------------------------- #
# Shaped output schemas — these are what FT-081 callers will use in slice 1   #
# (an ADR rationale, a structured verdict, etc.).                              #
# --------------------------------------------------------------------------- #


class AdrRationale(BaseModel):
    """Structured rationale output a verifier session would request."""

    verdict: str
    rationale: str
    cites: list[str] = []


class CodeChangeNote(BaseModel):
    summary: str
    files_touched: int


# --------------------------------------------------------------------------- #
# Fake LiteLLM response builders — shaped exactly like litellm.acompletion's   #
# OpenAI-compatible ModelResponse.                                             #
# --------------------------------------------------------------------------- #


class _Usage:
    def __init__(self, prompt: int, completion: int) -> None:
        self.prompt_tokens = prompt
        self.completion_tokens = completion
        self.total_tokens = prompt + completion


class _Message:
    def __init__(self, content: str) -> None:
        self.content = content
        self.role = "assistant"


class _Choice:
    def __init__(self, content: str) -> None:
        self.message = _Message(content)
        self.index = 0
        self.finish_reason = "stop"


class _Response:
    """Minimal LiteLLM/OpenAI-shaped object the SDK reads off."""

    def __init__(
        self,
        *,
        content: str,
        model: str,
        provider: str,
        prompt_tokens: int,
        completion_tokens: int,
        num_retries: int = 0,
    ) -> None:
        self.choices = [_Choice(content)]
        self.model = model
        self.usage = _Usage(prompt_tokens, completion_tokens)
        self._hidden_params = {
            "custom_llm_provider": provider,
            "num_retries": num_retries,
        }


# --------------------------------------------------------------------------- #
# Helpers — capture-and-respond fakes.                                         #
# --------------------------------------------------------------------------- #


def _capability_to_model_group(capability_tag: str) -> str:
    """Mirror what a real LiteLLM proxy config would route to.

    The SDK does NOT do this resolution — the proxy does. The fake
    captures the call's ``model=<tag>`` and returns a response whose
    ``model`` field is the resolved group, exactly as a real proxy would.
    """
    return {
        "frontier-reasoning": "anthropic/claude-opus-4-7",
        "code-writer": "scaleway/qwen3-coder-30b-a3b-instruct",
        "standard-reasoning": "scaleway/gpt-oss-120b",
    }.get(capability_tag, f"unknown/{capability_tag}")


def _capability_to_provider(capability_tag: str) -> str:
    return _capability_to_model_group(capability_tag).split("/", 1)[0]


class FakeCompletion:
    """Captures every ``acompletion`` call so the test can assert on it."""

    def __init__(self, response_content: str) -> None:
        self.calls: list[dict[str, Any]] = []
        self.response_content = response_content

    async def __call__(self, **kwargs: Any) -> _Response:
        self.calls.append(kwargs)
        # The proxy resolves the model group; mirror that in the response.
        model_param = kwargs["model"]
        return _Response(
            content=self.response_content,
            model=_capability_to_model_group(model_param),
            provider=_capability_to_provider(model_param),
            prompt_tokens=312,
            completion_tokens=78,
            num_retries=1,
        )


# --------------------------------------------------------------------------- #
# Criterion 1 — capability-tag → Pydantic instance round-trip.                 #
# --------------------------------------------------------------------------- #


async def test_capability_tag_resolves_and_returns_pydantic_instance() -> None:
    """``Provider.complete(capability_tag=…, output_schema=…)`` round-trips.

    The fake LiteLLM emits JSON for an ``AdrRationale``; the SDK coerces
    it to the Pydantic model via the structured-output adapter. Workers
    never see "anthropic/claude-opus-4-7" — they ask for the
    ``frontier-reasoning`` capability tag and the proxy resolves the rest.
    """
    fake = FakeCompletion(
        response_content=(
            '{"verdict":"approved",'
            '"rationale":"the patch satisfies TC-146 and ADR-054.",'
            '"cites":["TC-146","ADR-054"]}'
        )
    )
    provider = Provider(
        client=LiteLLMClient(
            config=LiteLLMConfig(base_url="http://proxy:4000", api_key="sk-virtual"),
            completion_fn=fake,
        )
    )

    result = await provider.complete(
        capability_tag="frontier-reasoning",
        messages=[
            {"role": "system", "content": "You are a verifier."},
            {"role": "user", "content": "Evaluate the patch."},
        ],
        output_schema=AdrRationale,
        metadata={"ddd_session_id": "urn:dec:session:abc-123"},
    )

    # The capability tag was sent to LiteLLM as ``model`` — the proxy
    # (mocked here) resolves the group; workers never see the model name.
    assert len(fake.calls) == 1
    call = fake.calls[0]
    assert call["model"] == "frontier-reasoning"
    assert call["api_base"] == "http://proxy:4000"
    assert call["api_key"] == "sk-virtual"
    assert call["messages"][0]["role"] == "system"
    assert call["messages"][1]["content"] == "Evaluate the patch."

    # The structured-output coercer produced a typed Pydantic instance.
    assert isinstance(result, CompletionResult)
    assert isinstance(result.output, AdrRationale)
    assert result.output.verdict == "approved"
    assert result.output.rationale.startswith("the patch satisfies")
    assert result.output.cites == ["TC-146", "ADR-054"]


# --------------------------------------------------------------------------- #
# Criterion 2 — synchronous telemetry: tokens, model, provider, latency,       #
# retry count are captured for attachment to the session.                      #
# --------------------------------------------------------------------------- #


async def test_synchronous_telemetry_captures_tokens_model_provider_retries() -> None:
    """FT-081's synchronous telemetry block is authoritative for provenance.

    The fake response carries ``model`` (resolved group), ``usage``
    (token counts) and ``_hidden_params`` (provider + retry count); the
    SDK pulls them off into a :class:`CompletionTelemetry` shape that
    merges into session telemetry.
    """
    fake = FakeCompletion('{"summary":"refactored bundle loader","files_touched":3}')
    provider = Provider(client=LiteLLMClient(completion_fn=fake))

    result = await provider.complete(
        capability_tag="code-writer",
        messages=[{"role": "user", "content": "Refactor."}],
        output_schema=CodeChangeNote,
        metadata={"ddd_session_id": "urn:dec:session:xyz-987"},
    )

    t = result.telemetry
    assert isinstance(t, CompletionTelemetry)
    assert t.capability_tag == "code-writer"
    assert t.model == "scaleway/qwen3-coder-30b-a3b-instruct"
    assert t.provider == "scaleway"
    assert t.input_tokens == 312
    assert t.output_tokens == 78
    assert t.total_tokens == 390
    assert t.retry_count == 1
    assert t.latency_seconds >= 0.0


async def test_telemetry_merges_into_session_completion_payload() -> None:
    """The Provider's telemetry slots into a Session and rides through to wire.

    FT-081 explicitly names ``Session`` integration: the synchronous
    telemetry must end up in the session's completion payload so the
    harness can reconcile it against LiteLLM's async cost callback.
    """
    fake = FakeCompletion('{"verdict":"approved","rationale":"ok ok ok ok ok","cites":[]}')
    provider = Provider(client=LiteLLMClient(completion_fn=fake))

    dispatch = DispatchEvent(
        event_id="42",
        dispatch_id="urn:dec:dispatch:ft-081",
        capability_tag="frontier-reasoning",
        nquads_payload="",
        metadata={"session_id": "urn:dec:session:tc-146"},
    )
    session = Session(dispatch)
    result = await provider.complete(
        capability_tag="frontier-reasoning",
        messages=[{"role": "user", "content": "Verify."}],
        output_schema=AdrRationale,
        metadata={"ddd_session_id": session.id},
    )
    result.telemetry.merge_into(session._telemetry)
    completion = session.build_completion()

    # The session's telemetry block now carries the provider-layer fields.
    assert completion.telemetry["capability_tag"] == "frontier-reasoning"
    assert completion.telemetry["model"] == "anthropic/claude-opus-4-7"
    assert completion.telemetry["provider"] == "anthropic"
    assert completion.telemetry["input_tokens"] == 312
    assert completion.telemetry["output_tokens"] == 78
    assert completion.telemetry["retry_count"] == 1
    # Identity is preserved across the merge (ADR-050).
    assert completion.session_id == "urn:dec:session:tc-146"


# --------------------------------------------------------------------------- #
# Criterion 3 — DDD session id propagated to LiteLLM metadata for callback     #
# correlation; ``extra_body`` and passthrough are forwarded untouched.         #
# --------------------------------------------------------------------------- #


async def test_metadata_threaded_through_for_callback_correlation() -> None:
    """LiteLLM's logging callback POSTs telemetry keyed on this metadata.

    The harness reconciles its async cost record against the worker's
    sync telemetry via the same ``ddd_session_id`` value, so the SDK
    MUST forward the metadata block verbatim into the LiteLLM call.
    """
    fake = FakeCompletion('{"verdict":"approved","rationale":"good enough","cites":[]}')
    provider = Provider(client=LiteLLMClient(completion_fn=fake))

    await provider.complete(
        capability_tag="frontier-reasoning",
        messages=[{"role": "user", "content": "test"}],
        output_schema=AdrRationale,
        metadata={
            "ddd_session_id": "urn:dec:session:correlate-1",
            "dispatch_id": "urn:dec:dispatch:correlate-1",
            "role_id": "verifier",
        },
        extra_body={"response_format": {"type": "json_object"}},
    )

    call = fake.calls[0]
    assert call["metadata"] == {
        "ddd_session_id": "urn:dec:session:correlate-1",
        "dispatch_id": "urn:dec:dispatch:correlate-1",
        "role_id": "verifier",
    }
    # ``extra_body`` is what provider-specific parameters ride in
    # (Anthropic tool use, OpenAI response_format) per ADR-054 — the SDK
    # forwards it without inspecting.
    assert call["extra_body"] == {"response_format": {"type": "json_object"}}


async def test_passthrough_kwargs_reach_litellm() -> None:
    fake = FakeCompletion('{"summary":"x","files_touched":0}')
    provider = Provider(client=LiteLLMClient(completion_fn=fake))
    await provider.complete(
        capability_tag="code-writer",
        messages=[{"role": "user", "content": "y"}],
        output_schema=CodeChangeNote,
        temperature=0.1,
        max_tokens=512,
    )
    call = fake.calls[0]
    assert call["temperature"] == 0.1
    assert call["max_tokens"] == 512


# --------------------------------------------------------------------------- #
# Criterion 4 — endpoint configurability via env vars per ADR-053.             #
# --------------------------------------------------------------------------- #


def test_default_endpoint_matches_adr_053() -> None:
    """Slice-1 local-host LiteLLM lives at ``http://localhost:4000``."""
    cfg = LiteLLMConfig.from_env({})
    assert cfg.base_url == DEFAULT_LITELLM_BASE_URL
    assert cfg.base_url == "http://localhost:4000"


def test_env_overrides_base_url_and_api_key() -> None:
    """Moving the proxy is an env-var change — never a code change."""
    cfg = LiteLLMConfig.from_env(
        {
            "LITELLM_BASE_URL": "http://sidecar:9000",
            "LITELLM_API_KEY": "sk-virtual-prod",
            "LITELLM_TIMEOUT": "30",
        }
    )
    assert cfg.base_url == "http://sidecar:9000"
    assert cfg.api_key == "sk-virtual-prod"
    assert cfg.timeout == 30.0


async def test_env_override_propagates_to_litellm_call() -> None:
    fake = FakeCompletion('{"verdict":"approved","rationale":"long enough","cites":[]}')
    cfg = LiteLLMConfig.from_env(
        {
            "LITELLM_BASE_URL": "http://moved-elsewhere:6000",
            "LITELLM_API_KEY": "sk-rotated",
        }
    )
    provider = Provider(client=LiteLLMClient(config=cfg, completion_fn=fake))
    await provider.complete(
        capability_tag="frontier-reasoning",
        messages=[{"role": "user", "content": "?"}],
        output_schema=AdrRationale,
    )
    call = fake.calls[0]
    assert call["api_base"] == "http://moved-elsewhere:6000"
    assert call["api_key"] == "sk-rotated"


# --------------------------------------------------------------------------- #
# Instructor-adapter path — production wiring uses ``structured_fn`` so the    #
# Pydantic instance is built inside the LiteLLM call (no JSON-coerce fallback).#
# --------------------------------------------------------------------------- #


async def test_structured_fn_path_returns_pydantic_directly() -> None:
    """The instructor-wired path returns a typed instance straight from the LLM.

    Real wiring binds ``structured_fn`` to ``instructor.from_litellm(...)``;
    tests inject a stub that returns ``(instance, raw_response)`` so the
    telemetry-extraction code path is exercised end-to-end.
    """
    captured: list[dict[str, Any]] = []

    async def fake_structured(**kwargs: Any) -> tuple[AdrRationale, _Response]:
        captured.append(kwargs)
        instance = AdrRationale(
            verdict="amendment-required",
            rationale="the patch is close but missing a cite",
            cites=["ADR-018"],
        )
        raw = _Response(
            content="<not used in structured path>",
            model="anthropic/claude-opus-4-7",
            provider="anthropic",
            prompt_tokens=900,
            completion_tokens=120,
            num_retries=0,
        )
        return instance, raw

    provider = Provider(
        client=LiteLLMClient(config=LiteLLMConfig(base_url="http://p:4000")),
        structured_fn=fake_structured,
    )
    result = await provider.complete(
        capability_tag="frontier-reasoning",
        messages=[{"role": "user", "content": "verify"}],
        output_schema=AdrRationale,
        metadata={"ddd_session_id": "urn:dec:session:structured"},
    )

    assert isinstance(result.output, AdrRationale)
    assert result.output.verdict == "amendment-required"
    assert result.telemetry.input_tokens == 900
    assert result.telemetry.output_tokens == 120
    assert result.telemetry.model == "anthropic/claude-opus-4-7"

    # ``response_model`` is what instructor consumes; metadata forwarded.
    call = captured[0]
    assert call["model"] == "frontier-reasoning"
    assert call["response_model"] is AdrRationale
    assert call["metadata"]["ddd_session_id"] == "urn:dec:session:structured"


# --------------------------------------------------------------------------- #
# Structural — public surface exports.                                         #
# --------------------------------------------------------------------------- #


def test_provider_surface_exposed_from_package_root() -> None:
    """The provider layer is importable from ``pipeline_worker_sdk`` directly."""
    from pipeline_worker_sdk import (
        Provider as _P,
        LiteLLMClient as _C,
        LiteLLMConfig as _Cfg,
        CompletionTelemetry as _T,
        CompletionResult as _R,
        DEFAULT_LITELLM_BASE_URL as _D,
    )

    assert _P is Provider
    assert _C is LiteLLMClient
    assert _Cfg is LiteLLMConfig
    assert _T is CompletionTelemetry
    assert _R is CompletionResult
    assert _D == "http://localhost:4000"


def test_no_per_provider_methods_on_provider_surface() -> None:
    """ADR-054 rejects per-provider methods.

    The Provider exposes one entry point, ``complete``. Provider-specific
    parameters ride through ``extra_body``. Mirrors the assertion in
    ADR-054: "The SDK doesn't add per-provider methods."
    """
    surface = {
        name for name in dir(Provider) if not name.startswith("_")
    }
    assert "complete" in surface
    forbidden = {"anthropic", "openai", "scaleway", "bedrock", "vertex"}
    leaked = surface & forbidden
    assert leaked == set(), f"per-provider methods leaked onto Provider: {leaked}"


# --------------------------------------------------------------------------- #
# Validation failure path — bad LLM output surfaces as a typed error.          #
# --------------------------------------------------------------------------- #


async def test_invalid_json_response_raises_validation_error() -> None:
    from pydantic import ValidationError

    fake = FakeCompletion("not even json")
    provider = Provider(client=LiteLLMClient(completion_fn=fake))
    with pytest.raises(ValidationError):
        await provider.complete(
            capability_tag="code-writer",
            messages=[{"role": "user", "content": "x"}],
            output_schema=CodeChangeNote,
        )
