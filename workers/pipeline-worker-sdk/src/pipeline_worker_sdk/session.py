"""One-dispatch-to-one-completion session lifecycle backed by pyoxigraph (FT-078)."""

from __future__ import annotations

import time
from collections.abc import Iterable
from types import TracebackType
from typing import Any

try:
    import pyoxigraph
except ImportError as exc:  # pragma: no cover - environment misconfig
    raise ImportError(
        "pyoxigraph is required for the pipeline-worker SDK session layer; "
        "install via `uv pip install pyoxigraph`."
    ) from exc

from ._session_io import (
    build_telemetry,
    derive_session_iri,
    dump_combined,
    dump_nquads,
    load_nquads,
    now_iso,
)
from .side_channel import (
    FeedbackEmission,
    build_feedback_quads,
    build_judgment_quads,
    emission_is_blocking,
    mint_feedback_iri,
    mint_judgment_iri,
)
from .types import CompletionPayload, DispatchEvent

OUTCOME_SUCCESS = "success"
OUTCOME_BLOCKED = "blocked"
OUTCOME_ESCALATED = "escalated"
OUTCOME_FAILED = "failed"


class Session:
    """One-dispatch-one-completion session backed by pyoxigraph (ADR-049, ADR-050).

    The Session IS the ``prov:Activity`` (ADR-050); ``self.id`` matches the
    URI the harness uses in its session record. Mechanical provenance
    triples on produced artifacts (``prov:wasGeneratedBy``,
    ``prov:wasAttributedTo``, ``prov:generatedAtTime``) are NOT emitted on
    the wire — the harness's GraphWriter materializes them from the
    session record at write time (FT-069 ships the SHACL, FT-073 enforces
    it). The worker only emits artifact triples + side-channel triples
    + telemetry.
    """

    def __init__(
        self,
        dispatch: DispatchEvent,
        *,
        agent_iri: str | None = None,
        session_iri: str | None = None,
    ) -> None:
        self._dispatch = dispatch
        self._session_iri = derive_session_iri(dispatch, session_iri)
        self._agent_iri = agent_iri
        self._started_at = now_iso()
        self._monotonic_start = time.monotonic()

        # Bundle store: holds the dispatch's N-Quads payload for the
        # session's lifetime; discarded when the session is dropped.
        self._bundle = pyoxigraph.Store()
        load_nquads(self._bundle, dispatch.nquads_payload)

        # Output sub-stores: artifact triples and side-channel triples
        # are kept separately so the blocked path can drop one without
        # losing the other.
        self._artifact = pyoxigraph.Store()
        self._side_channel = pyoxigraph.Store()

        self._telemetry: dict[str, Any] = {}
        self._counters: dict[str, float] = {}
        self._closed = False
        # FT-082: a blocking feedback emission forces ``build_completion``
        # to return ``outcome=blocked`` per ADR-025, regardless of caller
        # request. Tracked across multiple emissions in one session.
        self._blocking_feedback_emitted = False
        self._emitted_feedback_iris: list[str] = []
        self._emitted_judgment_iris: list[str] = []

    # ------------------------------------------------------------------ #
    # Identity                                                           #
    # ------------------------------------------------------------------ #

    @property
    def id(self) -> str:
        """The Session URI = the ``prov:Activity`` URI (ADR-050)."""
        return self._session_iri

    @property
    def dispatch_id(self) -> str:
        return self._dispatch.dispatch_id

    @property
    def capability_tag(self) -> str:
        return self._dispatch.capability_tag

    @property
    def agent_iri(self) -> str | None:
        return self._agent_iri

    @property
    def dispatch(self) -> DispatchEvent:
        return self._dispatch

    @property
    def closed(self) -> bool:
        """True once ``build_completion``/``build_blocked_completion`` ran."""
        return self._closed

    # ------------------------------------------------------------------ #
    # Bundle / artifact / side-channel access                            #
    # ------------------------------------------------------------------ #

    @property
    def store(self) -> pyoxigraph.Store:
        """The in-memory bundle sub-graph — read-only for the worker."""
        return self._bundle

    @property
    def bundle_size(self) -> int:
        return len(self._bundle)

    def emit_artifact_quads(self, quads: Iterable[pyoxigraph.Quad]) -> None:
        for quad in quads:
            self._artifact.add(quad)

    def emit_artifact_nquads(self, nquads: str) -> None:
        load_nquads(self._artifact, nquads)

    @property
    def artifact_size(self) -> int:
        return len(self._artifact)

    def emit_side_channel_quads(self, quads: Iterable[pyoxigraph.Quad]) -> None:
        for quad in quads:
            self._side_channel.add(quad)

    def emit_side_channel_nquads(self, nquads: str) -> None:
        load_nquads(self._side_channel, nquads)

    @property
    def side_channel_size(self) -> int:
        return len(self._side_channel)

    # ------------------------------------------------------------------ #
    # FT-082: structured side-channel APIs                               #
    # ------------------------------------------------------------------ #

    def record_emergent_judgment(
        self,
        *,
        decision: str,
        rationale: str,
        judgment_iri: str | None = None,
        recorded_at: str | None = None,
    ) -> str:
        """Record an in-authority emergent judgment as artifact metadata.

        Per FT-082 the judgment is attached to the produced artifact's
        emission set (not the side-channel) so it surfaces to the paired
        interpretation session's bundle alongside the main artifact.
        Returns the minted ``dec:EmergentJudgment`` IRI.
        """
        self._require_open()
        iri = judgment_iri or mint_judgment_iri()
        quads = build_judgment_quads(
            iri=iri,
            decision=decision,
            rationale=rationale,
            source_session_iri=self._session_iri,
            recorded_at=recorded_at,
        )
        self.emit_artifact_quads(quads)
        self._emitted_judgment_iris.append(iri)
        return iri

    def emit_feedback(
        self,
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
        """Emit a `dec:Feedback` artifact through the session's side-channel.

        Per ADR-025: a blocking emission (``blocking=True``, or the class
        default for ``gap``/``contradiction``/``unimplementable``) forces
        the session's next ``build_completion`` to return
        ``outcome=blocked`` and to drop the artifact triples. A
        non-blocking emission flows alongside the artifact on completion.

        Returns the minted ``dec:Feedback`` IRI.
        """
        self._require_open()
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
            source_session_iri=self._session_iri,
        )
        self.emit_side_channel_quads(quads)
        self._emitted_feedback_iris.append(iri)
        if emission_is_blocking(emission):
            self._blocking_feedback_emitted = True
        return iri

    @property
    def emitted_feedback_iris(self) -> tuple[str, ...]:
        """IRIs of every `dec:Feedback` emitted in this session."""
        return tuple(self._emitted_feedback_iris)

    @property
    def emitted_judgment_iris(self) -> tuple[str, ...]:
        """IRIs of every `dec:EmergentJudgment` recorded in this session."""
        return tuple(self._emitted_judgment_iris)

    @property
    def has_blocking_feedback(self) -> bool:
        """True iff at least one blocking feedback emission has occurred."""
        return self._blocking_feedback_emitted

    # ------------------------------------------------------------------ #
    # Telemetry                                                          #
    # ------------------------------------------------------------------ #

    def record_telemetry(self, key: str, value: Any) -> None:
        """Set a scalar telemetry field (overwrites prior value)."""
        self._telemetry[key] = value

    def update_telemetry(self, **fields: Any) -> None:
        """Bulk-set scalar telemetry fields."""
        self._telemetry.update(fields)

    def accumulate(self, key: str, delta: float) -> None:
        """Add ``delta`` to a numeric counter (tokens used, cost, latency)."""
        self._counters[key] = self._counters.get(key, 0) + delta

    def _final_telemetry(self) -> dict[str, Any]:
        return build_telemetry(
            base=self._telemetry,
            counters=self._counters,
            session_iri=self._session_iri,
            dispatch=self._dispatch,
            started_at=self._started_at,
            monotonic_start=self._monotonic_start,
            bundle=self._bundle,
            artifact=self._artifact,
            side_channel=self._side_channel,
        )

    # ------------------------------------------------------------------ #
    # Completion payload construction                                    #
    # ------------------------------------------------------------------ #

    def _require_open(self) -> None:
        if self._closed:
            raise RuntimeError(
                f"session {self._session_iri} already produced a completion"
            )

    def _resolve_outcome(self, requested: str | None) -> str:
        """Resolve completion outcome, honouring ADR-025 blocking semantics."""
        if self._blocking_feedback_emitted:
            return OUTCOME_BLOCKED
        if requested is None:
            return OUTCOME_SUCCESS
        return requested

    def build_completion(
        self, *, outcome: str | None = None
    ) -> CompletionPayload:
        """Construct the completion payload for FT-077 to POST.

        - No blocking feedback emitted → ``outcome=success`` with the
          union of artifact + side-channel triples in ``nquads_payload``.
        - A blocking feedback emission has occurred (FT-082, ADR-025) →
          ``outcome=blocked`` and the artifact triples are dropped from
          the payload (a half-formed artifact is worse than none);
          side-channel triples remain so the harness sees the feedback.

        An explicit ``outcome`` argument is honoured unless a blocking
        feedback emission has already forced the session into the blocked
        path — ADR-025 makes blocking-on-emission load-bearing.
        """
        self._require_open()
        self._closed = True
        effective = self._resolve_outcome(outcome)
        if effective == OUTCOME_BLOCKED and self._blocking_feedback_emitted:
            nquads = dump_nquads(self._side_channel)
        else:
            nquads = dump_combined(self._artifact, self._side_channel)
        return CompletionPayload(
            dispatch_id=self._dispatch.dispatch_id,
            session_id=self._session_iri,
            nquads_payload=nquads,
            outcome=effective,
            telemetry=self._final_telemetry(),
        )

    def build_blocked_completion(
        self,
        *,
        error: str | None = None,
        outcome: str = OUTCOME_BLOCKED,
    ) -> CompletionPayload:
        """Build a `blocked` (or `escalated`) completion after an exception.

        Preserves whatever side-channel triples were captured before the
        failure point; drops the artifact triples — a half-formed
        artifact is worse than none. Outcome defaults to ``blocked`` but
        callers can pass ``OUTCOME_ESCALATED`` or ``OUTCOME_FAILED``.
        """
        self._require_open()
        self._closed = True
        if error is not None:
            self._telemetry["error"] = error
        side_only = dump_nquads(self._side_channel)
        return CompletionPayload(
            dispatch_id=self._dispatch.dispatch_id,
            session_id=self._session_iri,
            nquads_payload=side_only,
            outcome=outcome,
            telemetry=self._final_telemetry(),
        )

    # ------------------------------------------------------------------ #
    # Context manager                                                    #
    # ------------------------------------------------------------------ #

    def __enter__(self) -> Session:
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> bool:
        # The async wire poster cannot be awaited from __exit__, so we
        # leave completion construction to the caller. If the caller
        # exited via an exception without closing the session, record
        # the exception in telemetry so the eventual blocked-completion
        # call carries the traceback summary.
        if exc is not None and not self._closed:
            self._telemetry.setdefault("uncaught_exception", repr(exc))
        return False


__all__ = [
    "OUTCOME_BLOCKED",
    "OUTCOME_ESCALATED",
    "OUTCOME_FAILED",
    "OUTCOME_SUCCESS",
    "Session",
]
