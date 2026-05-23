"""Shared SDK consumed by decision-cli workers."""

from .bundle import Authority, EscalationHint, Bundle
from .feedback import FEEDBACK_RECORD_SENTINEL, FeedbackClass, FeedbackEmission, emit_feedback
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

__all__ = [
    "FEEDBACK_RECORD_SENTINEL",
    "SCALEWAY_BASE_URL",
    "SCALEWAY_KEY_ENV",
    "Authority",
    "Bundle",
    "EscalationHint",
    "FeedbackClass",
    "FeedbackEmission",
    "ModelCaller",
    "ScalewayClientError",
    "build_client",
    "emit_feedback",
    "extract_reasoning_trace",
    "missing_key_error_or_none",
    "scaleway_chat_caller",
]
