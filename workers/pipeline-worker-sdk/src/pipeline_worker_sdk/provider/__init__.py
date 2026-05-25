"""Capability-tag dispatch via LiteLLM with structured Pydantic output."""

from .instructor_adapter import CompletionResult, Provider, StructuredFn
from .litellm_client import (
    DEFAULT_LITELLM_BASE_URL,
    CompletionFn,
    CompletionTelemetry,
    LiteLLMClient,
    LiteLLMConfig,
    LiteLLMResult,
)

__all__ = [
    "DEFAULT_LITELLM_BASE_URL",
    "CompletionFn",
    "CompletionResult",
    "CompletionTelemetry",
    "LiteLLMClient",
    "LiteLLMConfig",
    "LiteLLMResult",
    "Provider",
    "StructuredFn",
]
