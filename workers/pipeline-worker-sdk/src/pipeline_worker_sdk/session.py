"""One-dispatch-to-one-completion session lifecycle backed by pyoxigraph (FT-078)."""

from __future__ import annotations

import time
import uuid
from collections.abc import Iterable
from datetime import datetime, timezone
from types import TracebackType
from typing import Any

try:
    import pyoxigraph
except ImportError as exc:  # pragma: no cover - environment misconfig
    raise ImportError(
        "pyoxigraph is required for the pipeline-worker SDK session layer; "
        "install via `uv pip install pyoxigraph`."
    ) from exc

from .types import CompletionPayload, DispatchEvent

OUTCOME_SUCCESS = "success"
OUTCOME_BLOCKED = "blocked"
OUTCOME_ESCALATED = "escalated"
OUTCOME_FAILED = "failed"

_NQUADS = pyoxigraph.RdfFormat.N_QUADS


def _now_iso() -> str:
    """ISO-8601 UTC timestamp used for session start/end telemetry."""
    return datetime.now(timezone.utc).isoformat()


def _derive_session_iri(dispatch: DispatchEvent, explicit: str | None) -> str:
    """Pick the Session URI per ADR-050 (Session IS a prov:Activity).

    Priority: explicit argument > ``session_id`` from dispatch metadata >
    fall back to a freshly-minted urn:uuid IRI. The Session URI is the
    same identity the harness uses in its session record.
    """
    if explicit:
        return explicit
    metadata_session = dispatch.metadata.get("session_id") if dispatch.metadata else None
    if metadata_session:
        return str(metadata_session)
    return f"urn:dec:session:{uuid.uuid4()}"


def _load_nquads(store: pyoxigraph.Store, nquads: str) -> None:
    """Parse a (possibly empty) N-Quads string into ``store``."""
    if nquads and nquads.strip():
        store.load(input=nquads, format=_NQUADS)


def _dump_nquads(store: pyoxigraph.Store) -> str:
    """Serialize ``store`` back to N-Quads text (empty string if empty)."""
    if len(store) == 0:
        return ""
    raw = store.dump(format=_NQUADS)
    return raw.decode("utf-8") if isinstance(raw, (bytes, bytearray)) else str(raw)


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
        self._session_iri = _derive_session_iri(dispatch, session_iri)
        self._agent_iri = agent_iri
        self._started_at = _now_iso()
        self._monotonic_start = time.monotonic()

        # Bundle store: holds the dispatch's N-Quads payload for the
        # session's lifetime; discarded when the session is dropped.
        self._bundle = pyoxigraph.Store()
        _load_nquads(self._bundle, dispatch.nquads_payload)

        # Output sub-stores: artifact triples and side-channel triples
        # are kept separately so the blocked path can drop one without
        # losing the other.
        self._artifact = pyoxigraph.Store()
        self._side_channel = pyoxigraph.Store()

        self._telemetry: dict[str, Any] = {}
        self._counters: dict[str, float] = {}
        self._closed = False

    # ------------------------------------------------------------------ #
    # Identity
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
    # Bundle access
    # ------------------------------------------------------------------ #

    @property
    def store(self) -> pyoxigraph.Store:
        """The in-memory bundle sub-graph — read-only for the worker."""
        return self._bundle

    @property
    def bundle_size(self) -> int:
        return len(self._bundle)

    # ------------------------------------------------------------------ #
    # Artifact emission
    # ------------------------------------------------------------------ #

    def emit_artifact_quads(self, quads: Iterable[pyoxigraph.Quad]) -> None:
        for quad in quads:
            self._artifact.add(quad)

    def emit_artifact_nquads(self, nquads: str) -> None:
        _load_nquads(self._artifact, nquads)

    @property
    def artifact_size(self) -> int:
        return len(self._artifact)

    # ------------------------------------------------------------------ #
    # Side-channel emission (feedback, defects, capability requests …)
    # ------------------------------------------------------------------ #

    def emit_side_channel_quads(self, quads: Iterable[pyoxigraph.Quad]) -> None:
        for quad in quads:
            self._side_channel.add(quad)

    def emit_side_channel_nquads(self, nquads: str) -> None:
        _load_nquads(self._side_channel, nquads)

    @property
    def side_channel_size(self) -> int:
        return len(self._side_channel)

    # ------------------------------------------------------------------ #
    # Telemetry
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
        elapsed = max(0.0, time.monotonic() - self._monotonic_start)
        ended = _now_iso()
        return {
            **self._telemetry,
            **self._counters,
            "session_id": self._session_iri,
            "dispatch_id": self._dispatch.dispatch_id,
            "capability_tag": self._dispatch.capability_tag,
            "started_at": self._started_at,
            "ended_at": ended,
            "duration_seconds": elapsed,
            "bundle_quad_count": len(self._bundle),
            "artifact_quad_count": len(self._artifact),
            "side_channel_quad_count": len(self._side_channel),
        }

    # ------------------------------------------------------------------ #
    # Completion payload construction
    # ------------------------------------------------------------------ #

    def _dump_combined(self) -> str:
        """Combine artifact + side-channel quads into one N-Quads string."""
        if len(self._artifact) == 0 and len(self._side_channel) == 0:
            return ""
        combined = pyoxigraph.Store()
        for quad in self._artifact:
            combined.add(quad)
        for quad in self._side_channel:
            combined.add(quad)
        return _dump_nquads(combined)

    def build_completion(
        self, *, outcome: str = OUTCOME_SUCCESS
    ) -> CompletionPayload:
        """Construct the success completion payload for FT-077 to POST.

        Includes the union of artifact + side-channel triples in
        ``nquads_payload`` plus the merged telemetry block. Marks the
        session closed so a double-completion attempt is detectable.
        """
        if self._closed:
            raise RuntimeError(
                f"session {self._session_iri} already produced a completion"
            )
        self._closed = True
        return CompletionPayload(
            dispatch_id=self._dispatch.dispatch_id,
            session_id=self._session_iri,
            nquads_payload=self._dump_combined(),
            outcome=outcome,
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
        if self._closed:
            raise RuntimeError(
                f"session {self._session_iri} already produced a completion"
            )
        self._closed = True
        if error is not None:
            self._telemetry["error"] = error
        side_only = _dump_nquads(self._side_channel)
        return CompletionPayload(
            dispatch_id=self._dispatch.dispatch_id,
            session_id=self._session_iri,
            nquads_payload=side_only,
            outcome=outcome,
            telemetry=self._final_telemetry(),
        )

    # ------------------------------------------------------------------ #
    # Context manager
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
