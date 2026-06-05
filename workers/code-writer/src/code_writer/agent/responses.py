"""Response builders for the agent loop (FT-123)."""

from __future__ import annotations

import json
import re
from typing import Any

from .._runner_common import _make_code_change_iri
from ..models import (
    CodeChange,
    DispatchPayload,
    FileWrite,
    ToolCall,
    WorkerError,
    WorkerResponse,
    WorkerResponseUsage,
    WorkerTelemetry,
)


def extract_addressed_feedback(blob: str) -> list[str]:
    """Pull `addressed_feedback_iris` out of the agent's final-summary text.

    Locates the most-recent `<<DEC_ADDRESSED_FEEDBACK>>...<<END>>` block
    in `blob`, parses its JSON body, and returns the `iris` array. Any
    parse failure / missing block → empty list (the Rust accept path
    surfaces `WorkerIgnoredFeedback` for the operator).

    Args:
        blob: The final assistant message text.

    Returns:
        List of feedback IRIs the model claims to have addressed.
    """
    if not blob:
        return []
    pattern = re.compile(
        r"<<DEC_ADDRESSED_FEEDBACK>>\s*(\{.*?\})\s*<<END_DEC_ADDRESSED_FEEDBACK>>",
        re.DOTALL,
    )
    matches = list(pattern.finditer(blob))
    if not matches:
        return []
    raw = matches[-1].group(1)
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        return []
    iris = payload.get("iris") if isinstance(payload, dict) else None
    if not isinstance(iris, list):
        return []
    return [str(i) for i in iris if isinstance(i, str)]


def build_success_response(
    payload: DispatchPayload,
    file_writes: list[FileWrite],
    tool_calls: list[ToolCall],
    final_summary: str,
    latency: float,
    usage: WorkerResponseUsage | None = None,
) -> WorkerResponse:
    """Assemble the `status="ok"` response from the agent loop.

    Args:
        payload: The dispatch payload.
        file_writes: List of files written during the session.
        tool_calls: All tool calls made during the session.
        final_summary: The final assistant message text.
        latency: Total session duration in seconds.
        usage: FT-146 token-breakdown sum across every LiteLLM call in
            the dispatch. ``None`` when no LLM call surfaced a usage block.

    Returns:
        A successful WorkerResponse.
    """
    # FT-108: parse the agent's citation block when the bundle asked for one
    addressed = extract_addressed_feedback(final_summary) if payload.defect_feedback else []

    code_change = CodeChange(
        iri=_make_code_change_iri(payload.dispatch_id),
        feature_id=payload.feature_id,
        session_id=payload.session_id,
        files=file_writes,
        summary=final_summary,
        addressed_feedback_iris=addressed,
    )

    telemetry = WorkerTelemetry(
        turn_count=len(tool_calls),
        latency_seconds=latency,
        tool_calls=tool_calls,
        errors=[],
    )

    return WorkerResponse(
        dispatch_id=payload.dispatch_id,
        session_id=payload.session_id,
        status="ok",
        code_change=code_change,
        telemetry=telemetry,
        usage=usage,
    )


def build_error_response(
    payload: DispatchPayload,
    category: str,
    message: str,
    detail: str = "",
    retryable: bool = False,
    telemetry: WorkerTelemetry | None = None,
) -> WorkerResponse:
    """Build an error WorkerResponse.

    Args:
        payload: The dispatch payload.
        category: Error category (see WorkerError).
        message: Human-readable error message.
        detail: Additional error details.
        retryable: Whether the error is transient.
        telemetry: Optional telemetry data.

    Returns:
        An error WorkerResponse.
    """
    return WorkerResponse(
        dispatch_id=payload.dispatch_id,
        session_id=payload.session_id,
        status="error",
        error=WorkerError(
            category=category,  # type: ignore
            message=message,
            detail=detail,
            retryable=retryable,
        ),
        telemetry=telemetry or WorkerTelemetry(),
    )


def build_no_tools_response(payload: DispatchPayload) -> WorkerResponse:
    """Response returned when allowed_tools is empty (fail-closed)."""
    return build_error_response(
        payload,
        category="invalid_dispatch",
        message="no tools granted (allowed_tools is empty or no intersection with registry)",
        retryable=False,
    )


def build_max_turns_response(
    payload: DispatchPayload,
    tool_calls: list[ToolCall],
    latency: float,
    usage: WorkerResponseUsage | None = None,
) -> WorkerResponse:
    """Response returned when max_turns is exceeded.

    FT-146: carries accumulated ``usage`` so the harness still records the
    tokens spent before the timeout.
    """
    telemetry = WorkerTelemetry(
        turn_count=len(tool_calls),
        latency_seconds=latency,
        tool_calls=tool_calls,
        errors=[f"max_turns ({payload.max_turns}) exceeded"],
    )
    response = build_error_response(
        payload,
        category="timeout",
        message=f"max_turns ({payload.max_turns}) exceeded",
        retryable=True,
        telemetry=telemetry,
    )
    response.usage = usage
    return response


def build_missing_litellm_key_response(payload: DispatchPayload) -> WorkerResponse:
    """Response returned when LITELLM_API_KEY is missing."""
    return build_error_response(
        payload,
        category="invalid_dispatch",
        message="LITELLM_API_KEY environment variable is required",
        detail="Set LITELLM_API_KEY to a valid LiteLLM virtual key",
        retryable=False,
    )
