"""Build `claude -p` env overlay from a DispatchPayload's resolved endpoint.

FT-066 / ADR-033: the dispatcher pins `(endpoint, model_id)` onto the
dispatch payload. This module translates that pair into the right
``ANTHROPIC_*`` env vars for the ``claude -p`` subprocess so the same
binary can route to either the Anthropic API directly or through
y-router against Scaleway-hosted models.

Pure module: no I/O, no subprocess calls. The only inputs are the
payload and the current process environment.
"""

from __future__ import annotations

import os

from .models import DispatchPayload

#: Default y-router proxy address. Overridable via ``DEC_YROUTER_URL``.
DEFAULT_YROUTER_URL = "http://localhost:8787"

#: Anthropic env-var slots Claude Code consults per tier. Pinning all
#: five to the same Scaleway model id stops haiku/sonnet/opus aliases
#: from falling back to default Anthropic ids the proxy cannot translate.
_SCALEWAY_MODEL_VAR_SLOTS: tuple[str, ...] = (
    "ANTHROPIC_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "ANTHROPIC_SMALL_FAST_MODEL",
    "ANTHROPIC_DEFAULT_SONET_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
)


class EndpointConfigError(Exception):
    """Raised when endpoint configuration cannot be resolved into env vars.

    The caller maps this to a ``WorkerError(category="endpoint_config",
    retryable=False)`` before the subprocess spawn so the operator sees a
    structured failure rather than a downstream ``claude -p`` 401/403.
    """

    def __init__(self, category: str, message: str) -> None:
        super().__init__(message)
        #: Sub-category (e.g. ``missing_credentials``, ``unsupported_endpoint``).
        self.category = category
        #: Human-readable message surfaced as ``WorkerError.message``.
        self.message = message


def _build_anthropic_env(env: dict[str, str], payload: DispatchPayload) -> dict[str, str]:
    """Anthropic branch — pin ``ANTHROPIC_MODEL`` only, leave base URL alone."""
    env["ANTHROPIC_MODEL"] = payload.model_id
    return env


def _build_scaleway_env(env: dict[str, str], payload: DispatchPayload) -> dict[str, str]:
    """Scaleway branch — inject base URL, auth token, and pin all model slots."""
    api_key = env.get("SCW_SECRET_KEY", "").strip()
    if not api_key:
        raise EndpointConfigError(
            category="missing_credentials",
            message=(
                "SCW_SECRET_KEY not set; cannot dispatch to Scaleway endpoint"
            ),
        )
    proxy_url = env.get("DEC_YROUTER_URL", DEFAULT_YROUTER_URL).rstrip("/")
    env["ANTHROPIC_BASE_URL"] = proxy_url
    env["ANTHROPIC_AUTH_TOKEN"] = api_key
    for var in _SCALEWAY_MODEL_VAR_SLOTS:
        env[var] = payload.model_id
    return env


def claude_env_for(payload: DispatchPayload) -> dict[str, str]:
    """Build the env dict the ``claude -p`` subprocess should inherit.

    Returns a fresh copy of ``os.environ`` overlaid with the per-endpoint
    routing vars. Raises :class:`EndpointConfigError` when:

    * ``payload.endpoint == "scaleway"`` and ``SCW_SECRET_KEY`` is unset.
    * ``payload.endpoint`` is not one of ``scaleway`` | ``anthropic``.

    The function never touches the workspace, never spawns processes,
    and never blocks. The reachability probe (if any) is a separate
    concern handled by the caller.
    """
    env = os.environ.copy()
    if payload.endpoint == "anthropic":
        return _build_anthropic_env(env, payload)
    if payload.endpoint == "scaleway":
        return _build_scaleway_env(env, payload)
    raise EndpointConfigError(
        category="unsupported_endpoint",
        message=f"unknown endpoint {payload.endpoint!r}",
    )
