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

    if litellm is None:
        return build_error_response(
            payload,
            category="internal",
            message="litellm package not installed",
            detail="Run: uv add litellm",
            retryable=False,
        )

    # Main loop
    for turn in range(payload.max_turns):
        try:
            # Call LiteLLM
            response = litellm.completion(
                model=payload.model_id,
                messages=[{"role": "system", "content": system_prompt}] + messages,
                tools=list(available_tools.values()),
                api_key=litellm_key,
                base_url=litellm_base_url,
            )
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
            return build_success_response(
                payload, file_writes, tool_calls, final_text, latency
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
    return build_max_turns_response(payload, tool_calls, latency)
