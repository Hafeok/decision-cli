"""In-process LiteLLM-client agentic loop (FT-123 / ADR-069).

The loop:
1. Validates LITELLM_API_KEY is set
2. Intersects payload.allowed_tools with TOOL_REGISTRY (fail-closed on empty)
3. Calls litellm.completion with system prompt + user message
4. For each tool_use block: dispatch, collect result, thread back
5. Stops on end_turn or max_turns exceeded
6. Returns WorkerResponse with CodeChange or error
"""

from __future__ import annotations

import json
import os
import sys
import time
from pathlib import Path
from typing import Any

try:
    import litellm
except ImportError:
    litellm = None  # type: ignore

from ..models import DispatchPayload, FileWrite, ToolCall, WorkerResponse
from .prompts import render_system_prompt, render_user_message
from .responses import (
    build_error_response,
    build_max_turns_response,
    build_missing_litellm_key_response,
    build_no_tools_response,
    build_success_response,
)
from .tools import TOOL_REGISTRY, TOOL_SCHEMAS


def run_agent(payload: DispatchPayload) -> WorkerResponse:
    """Run the in-process LiteLLM-client agentic loop.

    Args:
        payload: The dispatch payload from the harness.

    Returns:
        WorkerResponse with either a CodeChange or error.
    """
    # Check LITELLM_API_KEY is set
    litellm_key = os.environ.get("LITELLM_API_KEY", "").strip()
    if not litellm_key:
        return build_missing_litellm_key_response(payload)

    # Intersect allowed_tools with registry (fail-closed)
    allowed_set = set(payload.allowed_tools)
    available_tools = {name: schema for name, schema in TOOL_SCHEMAS.items() if name in allowed_set}

    if not available_tools:
        return build_no_tools_response(payload)

    # Get LiteLLM base URL (default to localhost)
    litellm_base_url = os.environ.get("LITELLM_BASE_URL", "http://localhost:4000")

    # Prepare workspace
    workspace = Path(payload.workspace_path)
    workspace.mkdir(parents=True, exist_ok=True)

    # Render system prompt and user message
    system_prompt = render_system_prompt(payload)
    user_message = render_user_message(payload)

    # Initialize conversation
    messages: list[dict[str, Any]] = [
        {"role": "user", "content": user_message}
    ]

    # Track state
    tool_calls: list[ToolCall] = []
    file_writes: list[FileWrite] = []
    started = time.monotonic()
    final_text = ""
    # FT-146: accumulate token usage across every LiteLLM call so the
    # harness can persist a SessionRecord with the cluster-cell aggregate.
    usage_sum_base = 0
    usage_sum_cache_write = 0
    usage_sum_cache_hit = 0
    usage_sum_output = 0
    usage_observed = False

    if litellm is None:
        return build_error_response(
            payload,
            category="internal",
            message="litellm package not installed",
            detail="Run: uv add litellm",
            retryable=False,
        )

    # Proactive throttle threshold — sleep until the reset window when
    # remaining tokens OR remaining requests fall below this fraction of
    # the per-minute limit. 10% leaves headroom for one more turn's
    # bundle without tripping 429. Override via CODE_WRITER_RL_THRESHOLD_PCT.
    rl_threshold_pct = _read_int_env("CODE_WRITER_RL_THRESHOLD_PCT", 10)

    # Adapt the (endpoint, model_id) pair from the capability resolver
    # to LiteLLM's <provider>/<model> convention. Scaleway exposes
    # OpenAI-compatible inference, so route via the "openai/" prefix
    # which makes LiteLLM use the OpenAI protocol against base_url
    # (= the Scaleway endpoint). Anthropic-native bindings get the
    # "anthropic/" prefix. Already-prefixed model_ids pass through.
    if "/" in payload.model_id:
        litellm_model = payload.model_id
    elif payload.endpoint == "scaleway":
        litellm_model = f"openai/{payload.model_id}"
    elif payload.endpoint == "anthropic":
        litellm_model = f"anthropic/{payload.model_id}"
    else:
        litellm_model = payload.model_id

    # Main loop
    for turn in range(payload.max_turns):
        try:
            # Call LiteLLM
            # Witnessed on the FT-148 cluster runs: without an explicit
            # timeout a turn can block in ssl-read for 25+ minutes when
            # the endpoint queues or the connection dies. Bound every
            # call; a timeout raises and the turn's error handling (plus
            # the harness's cell retry) takes over.
            response = litellm.completion(
                model=litellm_model,
                messages=[{"role": "system", "content": system_prompt}] + messages,
                tools=list(available_tools.values()),
                api_key=litellm_key,
                base_url=litellm_base_url,
                timeout=float(os.environ.get("DEC_LLM_TURN_TIMEOUT_SECONDS", "240")),
                num_retries=2,
            )
            # Proactive throttle on Scaleway's x-ratelimit-* headers — back
            # off before the 429 hits when the per-minute window is nearly
            # exhausted. See docs/scaleway-rate-limits.md for the headers
            # contract. No-op on endpoints that don't surface the headers
            # (Anthropic, OpenAI direct, etc.).
            _maybe_throttle(_extract_rate_limit_headers(response))
            # FT-146: accumulate this call's usage. LiteLLM normalises
            # `prompt_tokens` / `completion_tokens` across providers; cache
            # fields are present only on Anthropic (Scaleway → 0).
            _usage = getattr(response, "usage", None)
            if _usage is not None:
                usage_observed = True
                _base = int(getattr(_usage, "prompt_tokens", 0) or 0)
                _out = int(getattr(_usage, "completion_tokens", 0) or 0)
                _cw = int(getattr(_usage, "cache_creation_input_tokens", 0) or 0)
                _ch = int(getattr(_usage, "cache_read_input_tokens", 0) or 0)
                # LiteLLM returns prompt_tokens as the FULL input count
                # (base + cache_write + cache_hit on Anthropic). Subtract
                # the cache components so input_tokens_base means the
                # uncached input only — matching FT-057's field semantics.
                usage_sum_base += max(0, _base - _cw - _ch)
                usage_sum_cache_write += _cw
                usage_sum_cache_hit += _ch
                usage_sum_output += _out
        except Exception as exc:
            latency = time.monotonic() - started
            from ..models import WorkerTelemetry

            telemetry_so_far = WorkerTelemetry(
                turn_count=len(tool_calls),
                latency_seconds=latency,
                tool_calls=tool_calls,
                errors=[str(exc)],
            )

            return build_error_response(
                payload,
                category="internal",
                message=f"LiteLLM call failed: {exc}",
                retryable=True,
                telemetry=telemetry_so_far,
            )

        # Extract response
        choice = response.choices[0]
        message = choice.message
        stop_reason = getattr(choice, "finish_reason", "stop")

        # Collect assistant message text
        assistant_text = getattr(message, "content", "") or ""
        final_text = assistant_text

        # Check for tool calls
        tool_uses = getattr(message, "tool_calls", None)

        if not tool_uses or stop_reason in ("stop", "end_turn"):
            # End of conversation
            latency = time.monotonic() - started
            usage = _build_usage_if_observed(
                usage_observed,
                usage_sum_base,
                usage_sum_cache_write,
                usage_sum_cache_hit,
                usage_sum_output,
            )
            return build_success_response(
                payload, file_writes, tool_calls, final_text, latency, usage
            )

        # Process tool calls
        assistant_msg = {
            "role": "assistant",
            "content": assistant_text,
            "tool_calls": tool_uses,
        }
        messages.append(assistant_msg)

        tool_results = []
        for tool_use in tool_uses:
            tool_name = tool_use.function.name
            try:
                tool_args = (
                    json.loads(tool_use.function.arguments)
                    if isinstance(tool_use.function.arguments, str)
                    else tool_use.function.arguments
                )
            except Exception:
                tool_args = {}

            # Dispatch to tool
            if tool_name not in TOOL_REGISTRY:
                result_content = f"unknown tool: {tool_name}"
                is_error = True
                file_write = None
            else:
                dispatcher = TOOL_REGISTRY[tool_name]
                result_content, is_error, file_write = dispatcher(workspace, tool_args)

            # Record tool call
            tool_calls.append(
                ToolCall(
                    name=tool_name,
                    arguments=tool_args,
                    result_status="error" if is_error else "ok",
                )
            )

            # Record file write if any
            if file_write is not None:
                file_writes.append(file_write)

            # Build tool result message
            tool_results.append(
                {
                    "role": "tool",
                    "tool_call_id": tool_use.id,
                    "content": result_content,
                }
            )

        # Append tool results to conversation
        messages.extend(tool_results)

    # Max turns exceeded
    latency = time.monotonic() - started
    usage = _build_usage_if_observed(
        usage_observed,
        usage_sum_base,
        usage_sum_cache_write,
        usage_sum_cache_hit,
        usage_sum_output,
    )
    return build_max_turns_response(payload, tool_calls, latency, usage)


def _build_usage_if_observed(
    observed: bool,
    base: int,
    cache_write: int,
    cache_hit: int,
    output: int,
):
    """FT-146: wrap accumulated counts in a ``WorkerResponseUsage`` when at
    least one LiteLLM call surfaced a ``response.usage`` block. Returns
    ``None`` when no call did (so the harness records
    ``dec:usageSource = "unreported"``)."""
    if not observed:
        return None
    from ..models import WorkerResponseUsage

    return WorkerResponseUsage(
        input_tokens_base=base,
        input_tokens_cache_write=cache_write,
        input_tokens_cache_hit=cache_hit,
        output_tokens=output,
    )


# ---------------------------------------------------------------------------
# Proactive rate-limit throttling against Scaleway's x-ratelimit-* headers.
# See docs/scaleway-rate-limits.md in the decision-cli repo for the contract.
# Defensive by construction: missing / malformed headers are treated as "no
# signal" and the loop proceeds unchanged.
# ---------------------------------------------------------------------------


def _read_int_env(name: str, default: int) -> int:
    """Read an int env var with a default. Defensive against malformed values."""
    raw = os.environ.get(name, "").strip()
    if not raw:
        return default
    try:
        return int(raw)
    except ValueError:
        return default


def _extract_rate_limit_headers(response) -> dict[str, str]:
    """Pull the raw HTTP response headers off a LiteLLM completion object.

    LiteLLM versions surface them differently — try the common access
    patterns in order, return an empty dict (no signal) when none match.
    Always lower-cases keys so callers can do case-insensitive lookups.
    """
    candidates = []
    hidden = getattr(response, "_hidden_params", None)
    if isinstance(hidden, dict):
        for key in ("additional_headers", "headers", "response_headers"):
            candidates.append(hidden.get(key))
    candidates.append(getattr(response, "response_headers", None))
    candidates.append(getattr(response, "headers", None))
    for c in candidates:
        if isinstance(c, dict) and c:
            return {str(k).lower(): str(v) for k, v in c.items()}
    return {}


def _parse_reset_duration(raw) -> float | None:
    """Parse Scaleway's x-ratelimit-reset-{tokens,requests} header value.

    Observed formats:
      "250ms" / "35ms" — sub-second resets when the window is nearly fresh.
      "1.5s" / "10s"   — second granularity when the window is older.
      "750"            — bare integer (assume milliseconds per Scaleway docs).
    Returns seconds as a float, or None when the value can't be parsed.
    """
    if raw is None:
        return None
    s = str(raw).strip().lower()
    if not s:
        return None
    try:
        if s.endswith("ms"):
            return float(s[:-2]) / 1000.0
        if s.endswith("s"):
            return float(s[:-1])
        # Bare number: docs example "35ms" / "250ms" — assume ms.
        return float(s) / 1000.0
    except ValueError:
        return None


def _parse_int(raw) -> int | None:
    if raw is None:
        return None
    try:
        return int(str(raw).strip())
    except ValueError:
        return None


def _maybe_throttle(headers: dict[str, str], threshold_pct: int = 10) -> None:
    """Sleep until the rate-limit window resets when remaining capacity
    for either dimension (tokens, requests) drops below `threshold_pct`%.

    Defensive: missing headers or unparseable values → no sleep. Logs the
    decision to stderr so operators can correlate with drive timing.
    """
    if not headers:
        return
    sleep_for: float = 0.0
    sleep_reason = ""
    for dim in ("tokens", "requests"):
        limit = _parse_int(headers.get(f"x-ratelimit-limit-{dim}"))
        remaining = _parse_int(headers.get(f"x-ratelimit-remaining-{dim}"))
        reset = _parse_reset_duration(headers.get(f"x-ratelimit-reset-{dim}"))
        if limit is None or remaining is None or limit <= 0:
            continue
        ratio_pct = (remaining * 100) / limit
        if ratio_pct >= threshold_pct:
            continue
        # Below threshold — sleep until the window resets. Cap at 75s so
        # a malformed header can't hang the loop indefinitely (the per-minute
        # window is 60s; a 75s cap is generous headroom).
        candidate = min(reset if reset and reset > 0 else 60.0, 75.0)
        if candidate > sleep_for:
            sleep_for = candidate
            sleep_reason = (
                f"{dim} remaining {remaining}/{limit} "
                f"({ratio_pct:.1f}% < {threshold_pct}% threshold); "
                f"sleeping {candidate:.2f}s for reset"
            )
    if sleep_for > 0:
        sys.stderr.write(f"agent throttle: {sleep_reason}\n")
        sys.stderr.flush()
        time.sleep(sleep_for)
