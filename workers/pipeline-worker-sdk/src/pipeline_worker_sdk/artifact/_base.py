"""Hand-written builder base providing SHACL-conformant commit semantics (FT-080)."""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import ClassVar


RDF_TYPE_IRI = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
DEC_NS = "https://decision-cli.dev/ns#"
DEC_BOUNDARY_ARTIFACT = f"{DEC_NS}BoundaryArtifact"
DEC_EXTERNAL_ORIGIN = f"{DEC_NS}external_origin"

_IRI_HOST_RE = re.compile(r"^[A-Za-z][A-Za-z0-9+.\-]*:")


def _looks_like_iri(value: str) -> bool:
    """Cheap structural check that ``value`` is a valid IRI (scheme present)."""
    return bool(_IRI_HOST_RE.match(value))


@dataclass(frozen=True)
class MotivationalDescriptor:
    """One sh:or motivational alternative inside a per-type shape."""

    predicate_local: str
    predicate_iri: str
    target_class_local: str
    target_class_iri: str

    def label(self) -> str:
        return f"{self.predicate_local} → dec:{self.target_class_local}"


class CommitError(ValueError):
    """Raised by ``Builder.commit()`` when SHACL-derived validation fails.

    Carries the shape name and the SHACL conformance message so the worker
    can surface it directly in a blocked-completion telemetry block or a
    structured feedback emission. This is the *defensive* validation tier
    (ADR-041); the harness's GraphWriter re-validates authoritatively on
    receive (FT-073).
    """

    def __init__(
        self,
        target_class_local: str,
        message: str,
        *,
        focus_iri: str,
    ) -> None:
        super().__init__(
            f"SHACL conformance failed for dec:{target_class_local} "
            f"<{focus_iri}>: {message}"
        )
        self.target_class_local = target_class_local
        self.focus_iri = focus_iri
        self.shacl_message = message


class BuilderBase:
    """Shared behaviour for every generated typed artifact builder.

    Generated subclasses provide:
      * ``TARGET_CLASS_IRI`` / ``TARGET_CLASS_LOCAL`` — identity of the
        per-type SHACL shape this builder validates against.
      * ``MOTIVATIONAL`` — declared motivational alternatives (the per-type
        ``sh:or`` minus the ``BoundaryArtifact`` branch).
      * ``ACCEPTS_BOUNDARY`` — True iff the shape's ``sh:or`` lists the
        ``BoundaryArtifact`` class-membership alternative.
      * ``SOURCE_SHAPE`` — relative path to the source shape TTL file.
      * ``_validate_required()`` — per-shape body-field cardinality check.
      * ``_motivational_state()`` — returns a dict mapping each motivational
        predicate local-name to True if any value was added in the builder.
      * ``_type_triples()`` — returns the per-shape body triples (the
        result of the legacy ``to_triples()`` method).

    The base class layers on top:
      * ``link_to(target_iri, *, predicate)`` for ad-hoc forward edges.
      * ``emit_triple(s, p, o)`` escape hatch with telemetry counter.
      * ``mark_boundary_artifact(*, external_origin)`` for shapes that
        accept the BoundaryArtifact alternative.
      * ``commit()`` — runs SHACL-derived validation and returns the full
        triple set ready for the harness to receive.
    """

    TARGET_CLASS_IRI: ClassVar[str] = ""
    TARGET_CLASS_LOCAL: ClassVar[str] = ""
    SOURCE_SHAPE: ClassVar[str] = ""
    ACCEPTS_BOUNDARY: ClassVar[bool] = False
    MOTIVATIONAL: ClassVar[tuple[MotivationalDescriptor, ...]] = ()

    def __init__(self, iri: str) -> None:
        if not iri:
            raise ValueError("artifact IRI must not be empty")
        self.iri: str = iri
        self._extra_triples: list[tuple[str, str, str]] = []
        self._escape_hatch_count: int = 0
        self._is_boundary_artifact: bool = False
        self._boundary_external_origin: str | None = None
        self._committed: bool = False

    # ------------------------------------------------------------------ #
    # Telemetry / escape-hatch state                                     #
    # ------------------------------------------------------------------ #

    @property
    def escape_hatch_count(self) -> int:
        """Number of ``emit_triple`` invocations on this builder.

        Surfaces in completion telemetry as
        ``artifact_escape_hatch_count`` when the session commits the
        builder via :meth:`pipeline_worker_sdk.Session.commit_artifact`.
        Persistent non-zero counts are a gap-surface signal: the typed
        surface does not yet cover what the worker is reaching for.
        """
        return self._escape_hatch_count

    @property
    def is_boundary_artifact(self) -> bool:
        return self._is_boundary_artifact

    @property
    def boundary_external_origin(self) -> str | None:
        return self._boundary_external_origin

    @property
    def committed(self) -> bool:
        return self._committed

    # ------------------------------------------------------------------ #
    # Public builder API                                                 #
    # ------------------------------------------------------------------ #

    def link_to(
        self, target_iri: str, *, predicate: str
    ) -> "BuilderBase":
        """Add an unstructured forward edge from this artifact.

        Convenience for SHACL-conformant cases the typed surface doesn't
        yet model — same role as :meth:`emit_triple` but pins the subject
        to ``self.iri``. Does *not* bump the escape-hatch counter (this is
        a typed link, just not field-specific).
        """
        if not target_iri:
            raise ValueError("target_iri must not be empty")
        if not predicate:
            raise ValueError("predicate must not be empty")
        if not _looks_like_iri(target_iri):
            raise ValueError(
                f"target_iri must be a fully qualified IRI; got {target_iri!r}"
            )
        if not _looks_like_iri(predicate):
            raise ValueError(
                f"predicate must be a fully qualified IRI; got {predicate!r}"
            )
        self._extra_triples.append((self.iri, predicate, target_iri))
        return self

    def emit_triple(
        self, subject: str, predicate: str, object_: str
    ) -> "BuilderBase":
        """Add a raw triple bypassing the typed surface.

        Use only for shape-conformant cases the typed builder doesn't yet
        cover (an unmodeled edge, a literal field outside the per-type
        shape). Each call increments :attr:`escape_hatch_count`, which
        surfaces in completion telemetry as a gap-surface signal
        analogous to ``bundle.raw_store`` (FT-079 / FT-080 success
        criterion 3).
        """
        if not subject or not predicate:
            raise ValueError("subject and predicate must be non-empty")
        self._extra_triples.append((subject, predicate, str(object_)))
        self._escape_hatch_count += 1
        return self

    def mark_boundary_artifact(
        self, *, external_origin: str
    ) -> "BuilderBase":
        """Declare this artifact as a BoundaryArtifact (ADR-040).

        Required for outputs whose motivational origin is *external
        reality* — sensing-action results, CI-posted submissions, initial
        requests from outside the orchestration graph. Satisfies the
        ``BoundaryArtifact`` branch of the per-type ``sh:or`` and exempts
        the artifact from the motivational-predicate requirement on
        :meth:`commit`. Raises if the shape does not accept boundary
        membership (e.g. ``dec:Dispatch``).
        """
        if not self.ACCEPTS_BOUNDARY:
            raise CommitError(
                self.TARGET_CLASS_LOCAL,
                f"dec:{self.TARGET_CLASS_LOCAL} shape does not accept the "
                f"BoundaryArtifact alternative; this artifact type is "
                f"never boundary-originated.",
                focus_iri=self.iri,
            )
        if not external_origin or not external_origin.strip():
            raise ValueError(
                "external_origin must be a non-empty string describing how "
                "the artifact entered the system (BoundaryArtifact requires "
                "auditability per ADR-040)."
            )
        self._is_boundary_artifact = True
        self._boundary_external_origin = external_origin
        return self

    # ------------------------------------------------------------------ #
    # Subclass-provided extension points                                 #
    # ------------------------------------------------------------------ #

    def _validate_required(self) -> None:
        """Lightweight required-field check; generated subclasses override."""
        return None

    def _motivational_state(self) -> dict[str, bool]:
        """Return ``{predicate_local: any_added}`` for motivational edges.

        Generated subclasses override; the default empty dict skips the
        motivational check (used by Dispatch et al. with no sh:or).
        """
        return {}

    def _type_triples(self) -> list[tuple[str, str, str]]:
        """Per-shape body triples; generated subclasses override."""
        return [(self.iri, RDF_TYPE_IRI, self.TARGET_CLASS_IRI)]

    # ------------------------------------------------------------------ #
    # commit() — the chokepoint                                          #
    # ------------------------------------------------------------------ #

    def _validate_motivational(self) -> None:
        """SHACL-derived check: at least one ``sh:or`` alternative satisfied.

        Mirrors the per-type shape's ``sh:or`` constraint composed in
        FT-072's shape files: a non-Dispatch artifact must either declare
        ``dec:BoundaryArtifact`` membership *or* set at least one of the
        motivational predicates the shape lists. Dispatch (and any future
        type with no ``sh:or``) skips this check.
        """
        descriptors = self.MOTIVATIONAL
        if not descriptors and not self.ACCEPTS_BOUNDARY:
            # No motivational/boundary constraint declared — Dispatch and friends.
            return
        if self._is_boundary_artifact:
            return
        state = self._motivational_state()
        if any(state.values()):
            return
        # No alternative satisfied — surface SHACL-style alternatives list.
        if descriptors:
            alts = ", ".join(d.label() for d in descriptors)
            if self.ACCEPTS_BOUNDARY:
                alts = f"BoundaryArtifact class membership, or {alts}"
        else:
            alts = "BoundaryArtifact class membership"
        raise CommitError(
            self.TARGET_CLASS_LOCAL,
            (
                "shape requires at least one motivational alternative to be "
                f"satisfied (one of: {alts}). "
                "Either set one of the motivational predicates above or call "
                "mark_boundary_artifact(external_origin=...) per ADR-040."
            ),
            focus_iri=self.iri,
        )

    def _boundary_triples(self) -> list[tuple[str, str, str]]:
        if not self._is_boundary_artifact:
            return []
        # BoundaryArtifact membership + the required dec:external_origin.
        return [
            (self.iri, RDF_TYPE_IRI, DEC_BOUNDARY_ARTIFACT),
            (
                self.iri,
                DEC_EXTERNAL_ORIGIN,
                self._boundary_external_origin or "",
            ),
        ]

    def commit(self) -> list[tuple[str, str, str]]:
        """Validate against the per-type SHACL shape and return the triples.

        Runs:
          1. per-shape required-field validation (``_validate_required``),
          2. SHACL ``sh:or`` motivational/boundary alternative check.

        On success, returns the union of:
          * the rdf:type + body triples emitted by the generated subclass,
          * any ``link_to`` / ``emit_triple`` escape-hatch triples,
          * the BoundaryArtifact membership triples if
            :meth:`mark_boundary_artifact` was called.

        Calling :meth:`commit` twice is a programming error. The harness's
        GraphWriter re-validates authoritatively on receive (ADR-041 /
        FT-073) — local validation here is the defensive tier.
        """
        if self._committed:
            raise RuntimeError(
                f"builder for <{self.iri}> already committed; one builder "
                "produces one artifact per FT-080's scope."
            )
        self._validate_required()
        self._validate_motivational()
        triples = list(self._type_triples())
        triples.extend(self._extra_triples)
        triples.extend(self._boundary_triples())
        self._committed = True
        return triples


__all__ = [
    "DEC_BOUNDARY_ARTIFACT",
    "DEC_EXTERNAL_ORIGIN",
    "DEC_NS",
    "RDF_TYPE_IRI",
    "BuilderBase",
    "CommitError",
    "MotivationalDescriptor",
]
