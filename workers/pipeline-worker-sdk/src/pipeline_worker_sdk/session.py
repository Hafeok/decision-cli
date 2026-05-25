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

from ._session_artifact import commit_builder_on
from ._session_completion import (
    OUTCOME_BLOCKED,
    OUTCOME_ESCALATED,
    OUTCOME_FAILED,
    OUTCOME_SUCCESS,
    build_blocked_completion as _build_blocked_completion,
    build_clean_completion as _build_clean_completion,
)
from ._session_io import (
    build_telemetry,
    derive_session_iri,
    load_nquads,
    now_iso,
)
from ._session_side_channel import emit_feedback_on, record_emergent_judgment_on
from .artifact import BuilderBase, CommitError
from .bundle import Bundle
from .types import CompletionPayload, DispatchEvent


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
        self._raw_store_access_count = 0
        # FT-080: aggregate escape-hatch usage across every builder the
        # session commits; surfaces on completion telemetry as
        # ``artifact_escape_hatch_count`` per the FT-080 spec.
        self._artifact_escape_hatch_count = 0
        self._artifact_commits: int = 0
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

    @property
    def raw_store_access_count(self) -> int:
        """Times the curated bundle facade exposed ``raw_store`` (FT-079)."""
        return self._raw_store_access_count

    def bundle(self, focal_iri: str) -> Bundle:
        """Curated read-only facade over the session's bundle sub-graph (FT-079).

        Wires ``raw_store`` access to a session counter so gap-surface
        signal aggregates per-session and surfaces on completion telemetry.
        """
        return Bundle(
            self._bundle, focal_iri, on_raw_store_access=self._bump_raw_store
        )

    def _bump_raw_store(self) -> None:
        self._raw_store_access_count += 1

    def emit_artifact_quads(self, quads: Iterable[pyoxigraph.Quad]) -> None:
        for quad in quads:
            self._artifact.add(quad)

    def emit_artifact_nquads(self, nquads: str) -> None:
        load_nquads(self._artifact, nquads)

    @property
    def artifact_size(self) -> int:
        return len(self._artifact)

    def commit_artifact(
        self,
        builder: BuilderBase,
        *,
        graph_iri: str | None = None,
    ) -> int:
        """Validate a typed artifact builder and stream its triples into the session (FT-080).

        Raises :class:`CommitError` if SHACL validation fails — workers
        catching this should typically transition to a blocked
        completion via :meth:`build_blocked_completion`.
        """
        return commit_builder_on(self, builder, graph_iri=graph_iri)

    @property
    def artifact_escape_hatch_count(self) -> int:
        """Cumulative escape-hatch invocations across all committed builders.

        Surfaces on the completion event's telemetry as
        ``artifact_escape_hatch_count``. A persistent non-zero count is a
        gap-surface signal (FT-080 success criterion 3) — the typed
        surface does not yet cover what the worker is reaching for.
        """
        return self._artifact_escape_hatch_count

    @property
    def artifact_commits(self) -> int:
        """Number of typed artifact builders the session has committed."""
        return self._artifact_commits

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
        """Record an in-authority emergent judgment as artifact metadata (FT-082)."""
        return record_emergent_judgment_on(
            self,
            decision=decision,
            rationale=rationale,
            judgment_iri=judgment_iri,
            recorded_at=recorded_at,
        )

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
        """Emit a `dec:Feedback` artifact through the session's side-channel (ADR-025)."""
        return emit_feedback_on(
            self,
            feedback_class=feedback_class,
            evidence=evidence,
            severity=severity,
            recommendation=recommendation,
            target_role=target_role,
            blocking=blocking,
            disposition_rationale=disposition_rationale,
            feedback_iri=feedback_iri,
        )

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
        block = build_telemetry(
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
        block["bundle_raw_store_access_count"] = self._raw_store_access_count
        # FT-080 success criterion 3: surface escape-hatch usage so the
        # harness can aggregate it as a gap-surface signal.
        block["artifact_escape_hatch_count"] = self._artifact_escape_hatch_count
        block["artifact_commits"] = self._artifact_commits
        return block

    # ------------------------------------------------------------------ #
    # Completion payload construction                                    #
    # ------------------------------------------------------------------ #

    def _require_open(self) -> None:
        if self._closed:
            raise RuntimeError(
                f"session {self._session_iri} already produced a completion"
            )

    def build_completion(
        self, *, outcome: str | None = None
    ) -> CompletionPayload:
        """Construct the completion payload for FT-077 to POST (ADR-025)."""
        return _build_clean_completion(self, outcome=outcome)

    def build_blocked_completion(
        self,
        *,
        error: str | None = None,
        outcome: str = OUTCOME_BLOCKED,
    ) -> CompletionPayload:
        """Build a ``blocked`` (or ``escalated``) completion after an exception."""
        return _build_blocked_completion(self, error=error, outcome=outcome)

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
