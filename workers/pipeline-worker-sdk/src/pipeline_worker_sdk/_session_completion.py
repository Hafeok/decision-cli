"""Completion-payload helpers for :class:`Session` (FT-078 / ADR-025)."""

from __future__ import annotations

from typing import Protocol

from ._session_io import dump_combined, dump_nquads
from .types import CompletionPayload


OUTCOME_SUCCESS = "success"
OUTCOME_BLOCKED = "blocked"
OUTCOME_ESCALATED = "escalated"
OUTCOME_FAILED = "failed"


class _SessionLike(Protocol):
    """Minimal Session surface the completion helpers consume."""

    _session_iri: str
    _blocking_feedback_emitted: bool
    _telemetry: dict
    _artifact: object
    _side_channel: object
    _closed: bool

    @property
    def dispatch(self): ...
    def _require_open(self) -> None: ...
    def _final_telemetry(self) -> dict: ...


def resolve_outcome(
    session: _SessionLike, requested: str | None
) -> str:
    """Resolve completion outcome, honouring ADR-025 blocking semantics."""
    if session._blocking_feedback_emitted:
        return OUTCOME_BLOCKED
    if requested is None:
        return OUTCOME_SUCCESS
    return requested


def build_clean_completion(
    session: _SessionLike, *, outcome: str | None = None
) -> CompletionPayload:
    """Construct the completion payload for FT-077 to POST.

    - No blocking feedback emitted → ``outcome=success`` with the union
      of artifact + side-channel triples in ``nquads_payload``.
    - A blocking feedback emission has occurred (FT-082, ADR-025) →
      ``outcome=blocked`` and the artifact triples are dropped from the
      payload (a half-formed artifact is worse than none); side-channel
      triples remain so the harness sees the feedback.

    An explicit ``outcome`` argument is honoured unless a blocking
    feedback emission has already forced the session into the blocked
    path — ADR-025 makes blocking-on-emission load-bearing.
    """
    session._require_open()
    session._closed = True
    effective = resolve_outcome(session, outcome)
    if effective == OUTCOME_BLOCKED and session._blocking_feedback_emitted:
        nquads = dump_nquads(session._side_channel)
    else:
        nquads = dump_combined(session._artifact, session._side_channel)
    return CompletionPayload(
        dispatch_id=session.dispatch.dispatch_id,
        session_id=session._session_iri,
        nquads_payload=nquads,
        outcome=effective,
        telemetry=session._final_telemetry(),
    )


def build_blocked_completion(
    session: _SessionLike,
    *,
    error: str | None = None,
    outcome: str = OUTCOME_BLOCKED,
) -> CompletionPayload:
    """Build a ``blocked`` (or ``escalated``) completion after an exception.

    Preserves whatever side-channel triples were captured before the
    failure point; drops the artifact triples — a half-formed artifact
    is worse than none. Outcome defaults to ``blocked`` but callers can
    pass :data:`OUTCOME_ESCALATED` or :data:`OUTCOME_FAILED`.
    """
    session._require_open()
    session._closed = True
    if error is not None:
        session._telemetry["error"] = error
    side_only = dump_nquads(session._side_channel)
    return CompletionPayload(
        dispatch_id=session.dispatch.dispatch_id,
        session_id=session._session_iri,
        nquads_payload=side_only,
        outcome=outcome,
        telemetry=session._final_telemetry(),
    )


__all__ = [
    "OUTCOME_BLOCKED",
    "OUTCOME_ESCALATED",
    "OUTCOME_FAILED",
    "OUTCOME_SUCCESS",
    "build_blocked_completion",
    "build_clean_completion",
    "resolve_outcome",
]
