"""Typed builder for dec:Brief artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/brief.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from .._base import BuilderBase, MotivationalDescriptor, RDF_TYPE_IRI


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Brief'


class BriefBuilder(BuilderBase):
    """Builder for emitting dec:Brief artifacts.

    Workers call ``add_*`` / ``set_*`` then ``commit()`` to obtain
    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.
    SHACL conformance is enforced locally on ``commit()`` (FT-080) and
    re-validated authoritatively on the harness side (ADR-041 / FT-073).
    The shared escape hatches ``emit_triple`` / ``link_to`` /
    ``mark_boundary_artifact`` come from :class:`BuilderBase`.
    """

    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI
    TARGET_CLASS_LOCAL: str = 'Brief'
    SOURCE_SHAPE: str = 'workers/_shared/shapes/brief.ttl'
    ACCEPTS_BOUNDARY: bool = True
    MOTIVATIONAL: tuple[MotivationalDescriptor, ...] = (
        MotivationalDescriptor(
            predicate_local='respondsTo',
            predicate_iri='https://decision-cli.dev/ns#respondsTo',
            target_class_local='Feedback',
            target_class_iri='https://decision-cli.dev/ns#Feedback',
        ),
    )

    P_goal: str = 'https://decision-cli.dev/ns#goal'
    P_premise: str = 'https://decision-cli.dev/ns#premise'
    P_successCriteria: str = 'https://decision-cli.dev/ns#successCriteria'
    P_title: str = 'https://decision-cli.dev/ns#title'
    P_acknowledges: str = 'https://decision-cli.dev/ns#acknowledges'
    P_decomposesInto: str = 'https://decision-cli.dev/ns#decomposesInto'
    P_excludes: str = 'https://decision-cli.dev/ns#excludes'
    P_references: str = 'https://decision-cli.dev/ns#references'
    P_respondsTo: str = 'https://decision-cli.dev/ns#respondsTo'

    def __init__(self, iri: str) -> None:
        super().__init__(iri)
        self._goal: str | None = None
        self._premise: str | None = None
        self._successCriteria: str | None = None
        self._title: str | None = None
        self._acknowledges: list[str] = []
        self._decomposesInto: list[str] = []
        self._excludes: list[str] = []
        self._references: list[str] = []
        self._respondsTo: list[str] = []

    def set_goal(self, value: str) -> "BriefBuilder":
        """Set the required body field ``goal``."""
        self._goal = value
        return self

    def set_premise(self, value: str) -> "BriefBuilder":
        """Set the required body field ``premise``."""
        self._premise = value
        return self

    def set_successCriteria(self, value: str) -> "BriefBuilder":
        """Set the required body field ``successCriteria``."""
        self._successCriteria = value
        return self

    def set_title(self, value: str) -> "BriefBuilder":
        """Set the required body field ``title``."""
        self._title = value
        return self

    def add_acknowledges(self, target_iri: str) -> "BriefBuilder":
        """Add forward edge ``acknowledges``."""
        self._acknowledges.append(target_iri)
        return self

    def add_decomposesInto(self, target_iri: str) -> "BriefBuilder":
        """Add forward edge ``decomposesInto``."""
        self._decomposesInto.append(target_iri)
        return self

    def add_excludes(self, target_iri: str) -> "BriefBuilder":
        """Add forward edge ``excludes``."""
        self._excludes.append(target_iri)
        return self

    def add_references(self, target_iri: str) -> "BriefBuilder":
        """Add forward edge ``references``."""
        self._references.append(target_iri)
        return self

    def add_respondsTo(self, target_iri: str) -> "BriefBuilder":
        """Add a motivational ``respondsTo`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._respondsTo.append(target_iri)
        return self

    def _validate_required(self) -> None:
        """Per-shape body-field cardinality check (FT-080 / ADR-041)."""
        if self._goal is None:
            from .._base import CommitError
            raise CommitError(
                self.TARGET_CLASS_LOCAL,
                "missing required body field: dec:goal",
                focus_iri=self.iri,
            )
        if self._premise is None:
            from .._base import CommitError
            raise CommitError(
                self.TARGET_CLASS_LOCAL,
                "missing required body field: dec:premise",
                focus_iri=self.iri,
            )
        if self._successCriteria is None:
            from .._base import CommitError
            raise CommitError(
                self.TARGET_CLASS_LOCAL,
                "missing required body field: dec:successCriteria",
                focus_iri=self.iri,
            )
        if self._title is None:
            from .._base import CommitError
            raise CommitError(
                self.TARGET_CLASS_LOCAL,
                "missing required body field: dec:title",
                focus_iri=self.iri,
            )

    def _motivational_state(self) -> dict[str, bool]:
        """Map ``{predicate_local: any_added}`` for SHACL ``sh:or`` evaluation."""
        return {
            'respondsTo': bool(self._respondsTo),
        }

    def _type_triples(self) -> list[tuple[str, str, str]]:
        """rdf:type + per-shape body triples (used by ``commit``)."""
        triples: list[tuple[str, str, str]] = []
        triples.append((self.iri, RDF_TYPE_IRI, self.TARGET_CLASS_IRI))
        if self._goal is not None:
            triples.append((self.iri, self.P_goal, str(self._goal)))
        if self._premise is not None:
            triples.append((self.iri, self.P_premise, str(self._premise)))
        if self._successCriteria is not None:
            triples.append((self.iri, self.P_successCriteria, str(self._successCriteria)))
        if self._title is not None:
            triples.append((self.iri, self.P_title, str(self._title)))
        for v in self._acknowledges:
            triples.append((self.iri, self.P_acknowledges, v))
        for v in self._decomposesInto:
            triples.append((self.iri, self.P_decomposesInto, v))
        for v in self._excludes:
            triples.append((self.iri, self.P_excludes, v))
        for v in self._references:
            triples.append((self.iri, self.P_references, v))
        for v in self._respondsTo:
            triples.append((self.iri, self.P_respondsTo, v))
        return triples

    def to_triples(self) -> list[tuple[str, str, str]]:
        """Backward-compatible accessor: returns the same triples
        as :meth:`commit` without enforcing SHACL ``sh:or``.

        New code should prefer :meth:`commit`, which raises on
        missing motivational / required fields per FT-080 success
        criterion 1.
        """
        self._validate_required()
        triples = list(self._type_triples())
        triples.extend(self._extra_triples)
        triples.extend(self._boundary_triples())
        return triples


__all__ = [
    "BriefBuilder",
    "TARGET_CLASS_IRI",
]
