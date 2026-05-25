"""Session ↔ artifact-builder integration helpers (FT-080)."""

from __future__ import annotations

from typing import Protocol

from ._session_io import triples_to_quads
from .artifact import BuilderBase


class _SessionLike(Protocol):
    """Minimal Session surface the artifact-commit helper consumes."""

    _artifact_escape_hatch_count: int
    _artifact_commits: int

    @property
    def dispatch(self): ...

    def _require_open(self) -> None: ...
    def emit_artifact_quads(self, quads) -> None: ...


def default_artifact_graph_iri(dispatch_id: str) -> str:
    """Per-dispatch named graph for committed artifact triples.

    Partitions artifact triples from the side-channel and the bundle on
    the wire so the harness's GraphWriter can route each named graph
    independently.
    """
    return f"urn:dec:artifact-graph:{dispatch_id}"


def commit_builder_on(
    session: _SessionLike,
    builder: BuilderBase,
    *,
    graph_iri: str | None = None,
) -> int:
    """Validate a typed artifact builder and stream its triples into the session.

    Calls :meth:`BuilderBase.commit` (which runs SHACL-derived validation
    per FT-080 success criterion 1) and writes the resulting triples
    into the session's artifact sub-store under a single named graph.
    Any escape-hatch usage on the builder (``emit_triple``) accumulates
    on the session's ``artifact_escape_hatch_count`` counter, which
    surfaces on the completion event (success criterion 3).
    """
    session._require_open()
    if not isinstance(builder, BuilderBase):
        raise TypeError(
            "commit_artifact() requires a BuilderBase subclass; got "
            f"{type(builder).__name__}"
        )
    triples = builder.commit()
    target_graph = graph_iri or default_artifact_graph_iri(
        session.dispatch.dispatch_id
    )
    quads = triples_to_quads(triples, target_graph)
    session.emit_artifact_quads(quads)
    if builder.escape_hatch_count:
        session._artifact_escape_hatch_count += builder.escape_hatch_count
    session._artifact_commits += 1
    return len(triples)


__all__ = [
    "commit_builder_on",
    "default_artifact_graph_iri",
]
