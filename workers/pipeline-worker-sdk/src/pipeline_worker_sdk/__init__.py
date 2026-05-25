"""Public surface re-exporting the pipeline-worker SDK's wire/session/provider layers."""

from .catalog import CatalogCache
from .claim import ClaimClient
from .poster import (
    CompletionFailed,
    CompletionPoster,
    CompletionRejected,
    RetryPolicy,
)
from .provider import (
    DEFAULT_LITELLM_BASE_URL,
    CompletionFn,
    CompletionResult,
    CompletionTelemetry,
    LiteLLMClient,
    LiteLLMConfig,
    LiteLLMResult,
    Provider,
    StructuredFn,
)
from .session import (
    OUTCOME_BLOCKED,
    OUTCOME_ESCALATED,
    OUTCOME_FAILED,
    OUTCOME_SUCCESS,
    Session,
)
from .sse import SseConsumer, envelope_to_dispatch, parse_sse_block
from .types import (
    CapabilityCatalogEntry,
    ClaimResult,
    CompletionPayload,
    DispatchEvent,
)
from .wire import HarnessEndpoints, WireClient

__all__ = [
    "DEFAULT_LITELLM_BASE_URL",
    "OUTCOME_BLOCKED",
    "OUTCOME_ESCALATED",
    "OUTCOME_FAILED",
    "OUTCOME_SUCCESS",
    "CapabilityCatalogEntry",
    "CatalogCache",
    "ClaimClient",
    "ClaimResult",
    "CompletionFailed",
    "CompletionFn",
    "CompletionPayload",
    "CompletionPoster",
    "CompletionRejected",
    "CompletionResult",
    "CompletionTelemetry",
    "DispatchEvent",
    "HarnessEndpoints",
    "LiteLLMClient",
    "LiteLLMConfig",
    "LiteLLMResult",
    "Provider",
    "RetryPolicy",
    "Session",
    "SseConsumer",
    "StructuredFn",
    "WireClient",
    "envelope_to_dispatch",
    "parse_sse_block",
]
