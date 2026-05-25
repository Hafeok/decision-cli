"""Internal I/O helpers for the Session layer — N-Quads parse, dump, identity, telemetry."""

from __future__ import annotations

import time
import uuid
from datetime import datetime, timezone
from typing import Any

import pyoxigraph

from .types import DispatchEvent

_NQUADS = pyoxigraph.RdfFormat.N_QUADS


def now_iso() -> str:
    """ISO-8601 UTC timestamp used for session start/end telemetry."""
    return datetime.now(timezone.utc).isoformat()


def derive_session_iri(dispatch: DispatchEvent, explicit: str | None) -> str:
    """Pick the Session URI per ADR-050 (Session IS a prov:Activity).

    Priority: explicit argument > ``session_id`` from dispatch metadata >
    fall back to a freshly-minted ``urn:dec:session:<uuid>`` IRI. The
    Session URI is the same identity the harness uses in its session
    record.
    """
    if explicit:
        return explicit
    metadata_session = (
        dispatch.metadata.get("session_id") if dispatch.metadata else None
    )
    if metadata_session:
        return str(metadata_session)
    return f"urn:dec:session:{uuid.uuid4()}"


def load_nquads(store: pyoxigraph.Store, nquads: str) -> None:
    """Parse a (possibly empty) N-Quads string into ``store``."""
    if nquads and nquads.strip():
        store.load(input=nquads, format=_NQUADS)


def dump_nquads(store: pyoxigraph.Store) -> str:
    """Serialize ``store`` back to N-Quads text (empty string if empty)."""
    if len(store) == 0:
        return ""
    raw = store.dump(format=_NQUADS)
    return raw.decode("utf-8") if isinstance(raw, (bytes, bytearray)) else str(raw)


def dump_combined(
    artifact: pyoxigraph.Store, side_channel: pyoxigraph.Store
) -> str:
    """Combine artifact + side-channel quads into one N-Quads string."""
    if len(artifact) == 0 and len(side_channel) == 0:
        return ""
    combined = pyoxigraph.Store()
    for quad in artifact:
        combined.add(quad)
    for quad in side_channel:
        combined.add(quad)
    return dump_nquads(combined)


def build_telemetry(
    *,
    base: dict[str, Any],
    counters: dict[str, float],
    session_iri: str,
    dispatch: DispatchEvent,
    started_at: str,
    monotonic_start: float,
    bundle: pyoxigraph.Store,
    artifact: pyoxigraph.Store,
    side_channel: pyoxigraph.Store,
) -> dict[str, Any]:
    """Compose the final telemetry block emitted on completion."""
    elapsed = max(0.0, time.monotonic() - monotonic_start)
    return {
        **base,
        **counters,
        "session_id": session_iri,
        "dispatch_id": dispatch.dispatch_id,
        "capability_tag": dispatch.capability_tag,
        "started_at": started_at,
        "ended_at": now_iso(),
        "duration_seconds": elapsed,
        "bundle_quad_count": len(bundle),
        "artifact_quad_count": len(artifact),
        "side_channel_quad_count": len(side_channel),
    }


__all__ = [
    "build_telemetry",
    "derive_session_iri",
    "dump_combined",
    "dump_nquads",
    "load_nquads",
    "now_iso",
]
