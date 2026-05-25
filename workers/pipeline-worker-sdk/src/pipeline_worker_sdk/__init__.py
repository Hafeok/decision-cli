"""Public surface of the pipeline-worker SDK wire layer (FT-077) plus session (FT-078)."""

from .catalog import CatalogCache
from .claim import ClaimClient
from .poster import (
    CompletionFailed,
    CompletionPoster,
    CompletionRejected,
    RetryPolicy,
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
    "OUTCOME_BLOCKED",
    "OUTCOME_ESCALATED",
    "OUTCOME_FAILED",
    "OUTCOME_SUCCESS",
    "CapabilityCatalogEntry",
    "CatalogCache",
    "ClaimClient",
    "ClaimResult",
    "CompletionFailed",
    "CompletionPayload",
    "CompletionPoster",
    "CompletionRejected",
    "DispatchEvent",
    "HarnessEndpoints",
    "RetryPolicy",
    "Session",
    "SseConsumer",
    "WireClient",
    "envelope_to_dispatch",
    "parse_sse_block",
]
