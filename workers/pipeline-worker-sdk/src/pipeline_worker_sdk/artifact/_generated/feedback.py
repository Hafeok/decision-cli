"""Typed builder for dec:Feedback artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/feedback.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from .._base import BuilderBase, MotivationalDescriptor, RDF_TYPE_IRI


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Feedback'


class FeedbackBuilder(BuilderBase):
    """Builder for emitting dec:Feedback artifacts.

    Workers call ``add_*`` / ``set_*`` then ``commit()`` to obtain
    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.
    SHACL conformance is enforced locally on ``commit()`` (FT-080) and
    re-validated authoritatively on the harness side (ADR-041 / FT-073).
    The shared escape hatches ``emit_triple`` / ``link_to`` /
    ``mark_boundary_artifact`` come from :class:`BuilderBase`.
    """

    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI
    TARGET_CLASS_LOCAL: str = 'Feedback'
    SOURCE_SHAPE: str = 'workers/_shared/shapes/feedback.ttl'
    ACCEPTS_BOUNDARY: bool = True
    MOTIVATIONAL: tuple[MotivationalDescriptor, ...] = (
        MotivationalDescriptor(
            predicate_local='observedIn',
            predicate_iri='https://decision-cli.dev/ns#observedIn',
            target_class_local='Session',
            target_class_iri='https://decision-cli.dev/ns#Session',
        ),
        MotivationalDescriptor(
            predicate_local='observedVia',
            predicate_iri='https://decision-cli.dev/ns#observedVia',
            target_class_local='SensingAction',
            target_class_iri='https://decision-cli.dev/ns#SensingAction',
        ),
        MotivationalDescriptor(
            predicate_local='producedBy',
            predicate_iri='https://decision-cli.dev/ns#producedBy',
            target_class_local='Role',
            target_class_iri='https://decision-cli.dev/ns#Role',
        ),
    )

    P_observedIn: str = 'https://decision-cli.dev/ns#observedIn'
    P_observedVia: str = 'https://decision-cli.dev/ns#observedVia'
    P_producedBy: str = 'https://decision-cli.dev/ns#producedBy'

    def __init__(self, iri: str) -> None:
        super().__init__(iri)
        self._observedIn: list[str] = []
        self._observedVia: list[str] = []
        self._producedBy: list[str] = []

    def add_observedIn(self, target_iri: str) -> "FeedbackBuilder":
        """Add a motivational ``observedIn`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._observedIn.append(target_iri)
        return self

    def add_observedVia(self, target_iri: str) -> "FeedbackBuilder":
        """Add a motivational ``observedVia`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._observedVia.append(target_iri)
        return self

    def add_producedBy(self, target_iri: str) -> "FeedbackBuilder":
        """Add a motivational ``producedBy`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._producedBy.append(target_iri)
        return self

    def _validate_required(self) -> None:
        """Per-shape body-field cardinality check (FT-080 / ADR-041)."""
        return None

    def _motivational_state(self) -> dict[str, bool]:
        """Map ``{predicate_local: any_added}`` for SHACL ``sh:or`` evaluation."""
        return {
            'observedIn': bool(self._observedIn),
            'observedVia': bool(self._observedVia),
            'producedBy': bool(self._producedBy),
        }

    def _type_triples(self) -> list[tuple[str, str, str]]:
        """rdf:type + per-shape body triples (used by ``commit``)."""
        triples: list[tuple[str, str, str]] = []
        triples.append((self.iri, RDF_TYPE_IRI, self.TARGET_CLASS_IRI))
        for v in self._observedIn:
            triples.append((self.iri, self.P_observedIn, v))
        for v in self._observedVia:
            triples.append((self.iri, self.P_observedVia, v))
        for v in self._producedBy:
            triples.append((self.iri, self.P_producedBy, v))
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
    "FeedbackBuilder",
    "TARGET_CLASS_IRI",
]
