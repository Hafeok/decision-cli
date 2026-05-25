"""Public surface re-exporting the pipeline-worker SDK's wire/session/provider layers."""

from .bundle import (
    ADR_TYPE_IRI,
    DEC_NS,
    TC_TYPE_IRI,
    Bundle,
    UnknownFocalTypeError,
)
from .catalog import CatalogCache
from .claim import ClaimClient
from .driver import Driver, EventDriver, FakeDispatch, FakeDriver
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
from .side_channel import (
    FEEDBACK_CLASSES,
    FeedbackClass,
    FeedbackEmission,
    build_feedback_quads,
    build_judgment_quads,
    mint_feedback_iri,
    mint_judgment_iri,
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
    "ADR_TYPE_IRI",
    "DEC_NS",
    "DEFAULT_LITELLM_BASE_URL",
    "FEEDBACK_CLASSES",
    "OUTCOME_BLOCKED",
    "OUTCOME_ESCALATED",
    "OUTCOME_FAILED",
    "OUTCOME_SUCCESS",
    "TC_TYPE_IRI",
    "Bundle",
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
    "Driver",
    "EventDriver",
    "FakeDispatch",
    "FakeDriver",
    "FeedbackClass",
    "FeedbackEmission",
    "HarnessEndpoints",
    "LiteLLMClient",
    "LiteLLMConfig",
    "LiteLLMResult",
    "Provider",
    "RetryPolicy",
    "Session",
    "SseConsumer",
    "StructuredFn",
    "UnknownFocalTypeError",
    "WireClient",
    "build_feedback_quads",
    "build_judgment_quads",
    "envelope_to_dispatch",
    "mint_feedback_iri",
    "mint_judgment_iri",
    "parse_sse_block",
]
