"""Structured side-channel helpers consumed by :class:`Session` (FT-082 / FT-080)."""

from __future__ import annotations

from typing import Protocol

from .side_channel import (
    FeedbackEmission,
    build_feedback_quads,
    build_judgment_quads,
    emission_is_blocking,
    mint_feedback_iri,
    mint_judgment_iri,
)


class _SessionLike(Protocol):
    """Minimal Session surface the side-channel helpers consume."""

    _session_iri: str
    _blocking_feedback_emitted: bool
    _emitted_feedback_iris: list[str]
    _emitted_judgment_iris: list[str]

    def _require_open(self) -> None: ...
    def emit_artifact_quads(self, quads) -> None: ...
    def emit_side_channel_quads(self, quads) -> None: ...


def record_emergent_judgment_on(
    session: _SessionLike,
    *,
    decision: str,
    rationale: str,
    judgment_iri: str | None = None,
    recorded_at: str | None = None,
) -> str:
    """Implement :meth:`Session.record_emergent_judgment` outside the class.

    Keeps session.py under the ADR-013 hard-line limit by moving the
    structured-emission body here. Behaviour is identical to the
    pre-FT-080 inline implementation; see that earlier docstring for
    semantics.
    """
    session._require_open()
    iri = judgment_iri or mint_judgment_iri()
    quads = build_judgment_quads(
        iri=iri,
        decision=decision,
        rationale=rationale,
        source_session_iri=session._session_iri,
        recorded_at=recorded_at,
    )
    session.emit_artifact_quads(quads)
    session._emitted_judgment_iris.append(iri)
    return iri


def emit_feedback_on(
    session: _SessionLike,
    *,
    feedback_class: str,
    evidence: str,
    severity: str = "warning",
    recommendation: str | None = None,
    target_role: str | None = None,
    blocking: bool | None = None,
    disposition_rationale: str | None = None,
    feedback_iri: str | None = None,
) -> str:
    """Implement :meth:`Session.emit_feedback` outside the class."""
    session._require_open()
    emission = FeedbackEmission(
        feedback_class=feedback_class,
        severity=severity,
        evidence=evidence,
        recommendation=recommendation,
        target_role=target_role,
        blocking=blocking,
        disposition_rationale=disposition_rationale,
    )
    iri = feedback_iri or mint_feedback_iri()
    quads = build_feedback_quads(
        iri=iri,
        emission=emission,
        source_session_iri=session._session_iri,
    )
    session.emit_side_channel_quads(quads)
    session._emitted_feedback_iris.append(iri)
    if emission_is_blocking(emission):
        session._blocking_feedback_emitted = True
    return iri


__all__ = [
    "emit_feedback_on",
    "record_emergent_judgment_on",
]
