"""Public surface of the pipeline-worker SDK wire layer (FT-077)."""

from .catalog import CatalogCache
from .claim import ClaimClient
from .poster import (
    CompletionFailed,
    CompletionPoster,
    CompletionRejected,
    RetryPolicy,
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
    "SseConsumer",
    "WireClient",
    "envelope_to_dispatch",
    "parse_sse_block",
]
