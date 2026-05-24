"""Endpoint error normalisation for ModelRouter (extracted from model_router.py).

Maps arbitrary SDK exceptions onto :class:`ModelRouterError` with an
``ErrorCategory`` derived from the exception's class name and message
text. Pulled into its own module to honour the ADR-013 file-length
limit on ``model_router.py``.
"""

from __future__ import annotations

from typing import Literal

from .scaleway_client import ScalewayClientError

ErrorCategory = Literal[
    "auth_failed",
    "rate_limited",
    "network_error",
    "invalid_response",
    "unknown",
]


class ModelRouterError(Exception):
    """Normalised endpoint error surfaced to worker callers."""

    def __init__(self, message: str, *, category: ErrorCategory = "unknown") -> None:
        super().__init__(message)
        self.category = category


_AUTH_HINTS = ("auth", "401", "unauthorized", "api key", "permission")
_RATE_HINTS = ("rate", "429", "too many", "quota")
_NETWORK_HINTS = ("timeout", "connection", "network", "unreachable", "dns")
_INVALID_HINTS = ("invalid", "400", "validation", "bad request", "schema")


def scaleway_error_to_router_error(exc: ScalewayClientError) -> ModelRouterError:
    """Map a Scaleway client error to the router's normalised shape."""
    category = "auth_failed" if exc.category == "missing_key" else "unknown"
    return ModelRouterError(str(exc), category=category)


def classify_endpoint_error(exc: Exception) -> ModelRouterError:
    """Map an arbitrary SDK error to a normalised :class:`ModelRouterError`."""
    if isinstance(exc, ModelRouterError):
        return exc
    category = _detect_error_category(exc)
    return ModelRouterError(str(exc) or exc.__class__.__name__, category=category)


def _detect_error_category(exc: Exception) -> ErrorCategory:
    """Inspect an exception for hints that map it to an :data:`ErrorCategory`."""
    class_name = exc.__class__.__name__.lower()
    message = str(exc).lower()
    haystack = f"{class_name} {message}"
    if any(h in haystack for h in _AUTH_HINTS):
        return "auth_failed"
    if any(h in haystack for h in _RATE_HINTS):
        return "rate_limited"
    if any(h in haystack for h in _NETWORK_HINTS):
        return "network_error"
    if any(h in haystack for h in _INVALID_HINTS):
        return "invalid_response"
    return "unknown"
