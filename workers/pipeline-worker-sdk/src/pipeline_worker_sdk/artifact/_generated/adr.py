"""Typed builder for dec:ADR artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/adr.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from .._base import BuilderBase, MotivationalDescriptor, RDF_TYPE_IRI


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#ADR'


class ADRBuilder(BuilderBase):
    """Builder for emitting dec:ADR artifacts.

    Workers call ``add_*`` / ``set_*`` then ``commit()`` to obtain
    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.
    SHACL conformance is enforced locally on ``commit()`` (FT-080) and
    re-validated authoritatively on the harness side (ADR-041 / FT-073).
    The shared escape hatches ``emit_triple`` / ``link_to`` /
    ``mark_boundary_artifact`` come from :class:`BuilderBase`.
    """

    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI
    TARGET_CLASS_LOCAL: str = 'ADR'
    SOURCE_SHAPE: str = 'workers/_shared/shapes/adr.ttl'
    ACCEPTS_BOUNDARY: bool = True
    MOTIVATIONAL: tuple[MotivationalDescriptor, ...] = (
        MotivationalDescriptor(
            predicate_local='addresses',
            predicate_iri='https://decision-cli.dev/ns#addresses',
            target_class_local='Question',
            target_class_iri='https://decision-cli.dev/ns#Question',
        ),
        MotivationalDescriptor(
            predicate_local='decidesFor',
            predicate_iri='https://decision-cli.dev/ns#decidesFor',
            target_class_local='Feature',
            target_class_iri='https://decision-cli.dev/ns#Feature',
        ),
        MotivationalDescriptor(
            predicate_local='supersedes',
            predicate_iri='https://decision-cli.dev/ns#supersedes',
            target_class_local='ADR',
            target_class_iri='https://decision-cli.dev/ns#ADR',
        ),
    )

    P_addresses: str = 'https://decision-cli.dev/ns#addresses'
    P_decidesFor: str = 'https://decision-cli.dev/ns#decidesFor'
    P_supersedes: str = 'https://decision-cli.dev/ns#supersedes'

    def __init__(self, iri: str) -> None:
        super().__init__(iri)
        self._addresses: list[str] = []
        self._decidesFor: list[str] = []
        self._supersedes: list[str] = []

    def add_addresses(self, target_iri: str) -> "ADRBuilder":
        """Add a motivational ``addresses`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._addresses.append(target_iri)
        return self

    def add_decidesFor(self, target_iri: str) -> "ADRBuilder":
        """Add a motivational ``decidesFor`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._decidesFor.append(target_iri)
        return self

    def add_supersedes(self, target_iri: str) -> "ADRBuilder":
        """Add a motivational ``supersedes`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._supersedes.append(target_iri)
        return self

    def _validate_required(self) -> None:
        """Per-shape body-field cardinality check (FT-080 / ADR-041)."""
        return None

    def _motivational_state(self) -> dict[str, bool]:
        """Map ``{predicate_local: any_added}`` for SHACL ``sh:or`` evaluation."""
        return {
            'addresses': bool(self._addresses),
            'decidesFor': bool(self._decidesFor),
            'supersedes': bool(self._supersedes),
        }

    def _type_triples(self) -> list[tuple[str, str, str]]:
        """rdf:type + per-shape body triples (used by ``commit``)."""
        triples: list[tuple[str, str, str]] = []
        triples.append((self.iri, RDF_TYPE_IRI, self.TARGET_CLASS_IRI))
        for v in self._addresses:
            triples.append((self.iri, self.P_addresses, v))
        for v in self._decidesFor:
            triples.append((self.iri, self.P_decidesFor, v))
        for v in self._supersedes:
            triples.append((self.iri, self.P_supersedes, v))
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
    "ADRBuilder",
    "TARGET_CLASS_IRI",
]
