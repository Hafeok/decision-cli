"""Shared SDK consumed by decision-cli workers."""

from .bundle import Authority, EscalationHint, Bundle
from .feedback import FEEDBACK_RECORD_SENTINEL, FeedbackClass, FeedbackEmission, emit_feedback
from .model_router import (
    ANTHROPIC_STRUCTURED_OUTPUT_TOOL,
    AnthropicRouter,
    CallParams,
    Endpoint,
    ModelResponse,
    ModelRouter,
    ModelRouterError,
    ReasoningEffort,
    ScalewayRouter,
    ToolCall,
    build_router,
)
from .scaleway_client import (
    SCALEWAY_BASE_URL,
    SCALEWAY_KEY_ENV,
    ModelCaller,
    ScalewayClientError,
    build_client,
    extract_reasoning_trace,
    missing_key_error_or_none,
    scaleway_chat_caller,
)
from .tools import (
    IMPLEMENTER_TOOLS,
    openai_tool_to_anthropic,
    translate_tools_for_anthropic,
)

__all__ = [
    "ANTHROPIC_STRUCTURED_OUTPUT_TOOL",
    "AnthropicRouter",
    "Authority",
    "Bundle",
    "CallParams",
    "Endpoint",
    "EscalationHint",
    "FEEDBACK_RECORD_SENTINEL",
    "FeedbackClass",
    "FeedbackEmission",
    "IMPLEMENTER_TOOLS",
    "ModelCaller",
    "ModelResponse",
    "ModelRouter",
    "ModelRouterError",
    "ReasoningEffort",
    "SCALEWAY_BASE_URL",
    "SCALEWAY_KEY_ENV",
    "ScalewayClientError",
    "ScalewayRouter",
    "ToolCall",
    "build_client",
    "build_router",
    "emit_feedback",
    "extract_reasoning_trace",
    "missing_key_error_or_none",
    "openai_tool_to_anthropic",
    "scaleway_chat_caller",
    "translate_tools_for_anthropic",
]
