"""Typed builder for dec:DiscoveryFinding artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/discovery-finding.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from .._base import BuilderBase, MotivationalDescriptor, RDF_TYPE_IRI


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#DiscoveryFinding'


class DiscoveryFindingBuilder(BuilderBase):
    """Builder for emitting dec:DiscoveryFinding artifacts.

    Workers call ``add_*`` / ``set_*`` then ``commit()`` to obtain
    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.
    SHACL conformance is enforced locally on ``commit()`` (FT-080) and
    re-validated authoritatively on the harness side (ADR-041 / FT-073).
    The shared escape hatches ``emit_triple`` / ``link_to`` /
    ``mark_boundary_artifact`` come from :class:`BuilderBase`.
    """

    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI
    TARGET_CLASS_LOCAL: str = 'DiscoveryFinding'
    SOURCE_SHAPE: str = 'workers/_shared/shapes/discovery-finding.ttl'
    ACCEPTS_BOUNDARY: bool = True
    MOTIVATIONAL: tuple[MotivationalDescriptor, ...] = (
        MotivationalDescriptor(
            predicate_local='derivedFrom',
            predicate_iri='https://decision-cli.dev/ns#derivedFrom',
            target_class_local='SensingAction',
            target_class_iri='https://decision-cli.dev/ns#SensingAction',
        ),
    )

    P_derivedFrom: str = 'https://decision-cli.dev/ns#derivedFrom'

    def __init__(self, iri: str) -> None:
        super().__init__(iri)
        self._derivedFrom: list[str] = []

    def add_derivedFrom(self, target_iri: str) -> "DiscoveryFindingBuilder":
        """Add a motivational ``derivedFrom`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._derivedFrom.append(target_iri)
        return self

    def _validate_required(self) -> None:
        """Per-shape body-field cardinality check (FT-080 / ADR-041)."""
        return None

    def _motivational_state(self) -> dict[str, bool]:
        """Map ``{predicate_local: any_added}`` for SHACL ``sh:or`` evaluation."""
        return {
            'derivedFrom': bool(self._derivedFrom),
        }

    def _type_triples(self) -> list[tuple[str, str, str]]:
        """rdf:type + per-shape body triples (used by ``commit``)."""
        triples: list[tuple[str, str, str]] = []
        triples.append((self.iri, RDF_TYPE_IRI, self.TARGET_CLASS_IRI))
        for v in self._derivedFrom:
            triples.append((self.iri, self.P_derivedFrom, v))
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
    "DiscoveryFindingBuilder",
    "TARGET_CLASS_IRI",
]
