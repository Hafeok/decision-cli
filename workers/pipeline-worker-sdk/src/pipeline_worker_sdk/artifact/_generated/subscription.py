"""Typed builder for dec:Subscription artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/subscription.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from .._base import BuilderBase, MotivationalDescriptor, RDF_TYPE_IRI


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Subscription'


class SubscriptionBuilder(BuilderBase):
    """Builder for emitting dec:Subscription artifacts.

    Workers call ``add_*`` / ``set_*`` then ``commit()`` to obtain
    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.
    SHACL conformance is enforced locally on ``commit()`` (FT-080) and
    re-validated authoritatively on the harness side (ADR-041 / FT-073).
    The shared escape hatches ``emit_triple`` / ``link_to`` /
    ``mark_boundary_artifact`` come from :class:`BuilderBase`.
    """

    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI
    TARGET_CLASS_LOCAL: str = 'Subscription'
    SOURCE_SHAPE: str = 'workers/_shared/shapes/subscription.ttl'
    ACCEPTS_BOUNDARY: bool = True
    MOTIVATIONAL: tuple[MotivationalDescriptor, ...] = (
        MotivationalDescriptor(
            predicate_local='motivatedBy',
            predicate_iri='https://decision-cli.dev/ns#motivatedBy',
            target_class_local='ADR',
            target_class_iri='https://decision-cli.dev/ns#ADR',
        ),
    )

    P_motivatedBy: str = 'https://decision-cli.dev/ns#motivatedBy'

    def __init__(self, iri: str) -> None:
        super().__init__(iri)
        self._motivatedBy: list[str] = []

    def add_motivatedBy(self, target_iri: str) -> "SubscriptionBuilder":
        """Add a motivational ``motivatedBy`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._motivatedBy.append(target_iri)
        return self

    def _validate_required(self) -> None:
        """Per-shape body-field cardinality check (FT-080 / ADR-041)."""
        return None

    def _motivational_state(self) -> dict[str, bool]:
        """Map ``{predicate_local: any_added}`` for SHACL ``sh:or`` evaluation."""
        return {
            'motivatedBy': bool(self._motivatedBy),
        }

    def _type_triples(self) -> list[tuple[str, str, str]]:
        """rdf:type + per-shape body triples (used by ``commit``)."""
        triples: list[tuple[str, str, str]] = []
        triples.append((self.iri, RDF_TYPE_IRI, self.TARGET_CLASS_IRI))
        for v in self._motivatedBy:
            triples.append((self.iri, self.P_motivatedBy, v))
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
    "SubscriptionBuilder",
    "TARGET_CLASS_IRI",
]
