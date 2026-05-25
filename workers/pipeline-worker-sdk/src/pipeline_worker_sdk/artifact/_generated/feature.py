"""Typed builder for dec:Feature artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/feature.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from .._base import BuilderBase, MotivationalDescriptor, RDF_TYPE_IRI


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Feature'


class FeatureBuilder(BuilderBase):
    """Builder for emitting dec:Feature artifacts.

    Workers call ``add_*`` / ``set_*`` then ``commit()`` to obtain
    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.
    SHACL conformance is enforced locally on ``commit()`` (FT-080) and
    re-validated authoritatively on the harness side (ADR-041 / FT-073).
    The shared escape hatches ``emit_triple`` / ``link_to`` /
    ``mark_boundary_artifact`` come from :class:`BuilderBase`.
    """

    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI
    TARGET_CLASS_LOCAL: str = 'Feature'
    SOURCE_SHAPE: str = 'workers/_shared/shapes/feature.ttl'
    ACCEPTS_BOUNDARY: bool = True
    MOTIVATIONAL: tuple[MotivationalDescriptor, ...] = (
        MotivationalDescriptor(
            predicate_local='addresses',
            predicate_iri='https://decision-cli.dev/ns#addresses',
            target_class_local='Feedback',
            target_class_iri='https://decision-cli.dev/ns#Feedback',
        ),
        MotivationalDescriptor(
            predicate_local='decomposesFrom',
            predicate_iri='https://decision-cli.dev/ns#decomposesFrom',
            target_class_local='Brief',
            target_class_iri='https://decision-cli.dev/ns#Brief',
        ),
        MotivationalDescriptor(
            predicate_local='originatedFrom',
            predicate_iri='https://decision-cli.dev/ns#originatedFrom',
            target_class_local='DiscoveryFinding',
            target_class_iri='https://decision-cli.dev/ns#DiscoveryFinding',
        ),
        MotivationalDescriptor(
            predicate_local='respondsTo',
            predicate_iri='https://decision-cli.dev/ns#respondsTo',
            target_class_local='Question',
            target_class_iri='https://decision-cli.dev/ns#Question',
        ),
    )

    P_addresses: str = 'https://decision-cli.dev/ns#addresses'
    P_decomposesFrom: str = 'https://decision-cli.dev/ns#decomposesFrom'
    P_originatedFrom: str = 'https://decision-cli.dev/ns#originatedFrom'
    P_respondsTo: str = 'https://decision-cli.dev/ns#respondsTo'

    def __init__(self, iri: str) -> None:
        super().__init__(iri)
        self._addresses: list[str] = []
        self._decomposesFrom: list[str] = []
        self._originatedFrom: list[str] = []
        self._respondsTo: list[str] = []

    def add_addresses(self, target_iri: str) -> "FeatureBuilder":
        """Add a motivational ``addresses`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._addresses.append(target_iri)
        return self

    def add_decomposesFrom(self, target_iri: str) -> "FeatureBuilder":
        """Add a motivational ``decomposesFrom`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._decomposesFrom.append(target_iri)
        return self

    def add_originatedFrom(self, target_iri: str) -> "FeatureBuilder":
        """Add a motivational ``originatedFrom`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._originatedFrom.append(target_iri)
        return self

    def add_respondsTo(self, target_iri: str) -> "FeatureBuilder":
        """Add a motivational ``respondsTo`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._respondsTo.append(target_iri)
        return self

    def _validate_required(self) -> None:
        """Per-shape body-field cardinality check (FT-080 / ADR-041)."""
        return None

    def _motivational_state(self) -> dict[str, bool]:
        """Map ``{predicate_local: any_added}`` for SHACL ``sh:or`` evaluation."""
        return {
            'addresses': bool(self._addresses),
            'decomposesFrom': bool(self._decomposesFrom),
            'originatedFrom': bool(self._originatedFrom),
            'respondsTo': bool(self._respondsTo),
        }

    def _type_triples(self) -> list[tuple[str, str, str]]:
        """rdf:type + per-shape body triples (used by ``commit``)."""
        triples: list[tuple[str, str, str]] = []
        triples.append((self.iri, RDF_TYPE_IRI, self.TARGET_CLASS_IRI))
        for v in self._addresses:
            triples.append((self.iri, self.P_addresses, v))
        for v in self._decomposesFrom:
            triples.append((self.iri, self.P_decomposesFrom, v))
        for v in self._originatedFrom:
            triples.append((self.iri, self.P_originatedFrom, v))
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
    "FeatureBuilder",
    "TARGET_CLASS_IRI",
]
