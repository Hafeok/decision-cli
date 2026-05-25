"""Typed builder for dec:QueryTemplate artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/query-template.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from .._base import BuilderBase, MotivationalDescriptor, RDF_TYPE_IRI


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#QueryTemplate'


class QueryTemplateBuilder(BuilderBase):
    """Builder for emitting dec:QueryTemplate artifacts.

    Workers call ``add_*`` / ``set_*`` then ``commit()`` to obtain
    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.
    SHACL conformance is enforced locally on ``commit()`` (FT-080) and
    re-validated authoritatively on the harness side (ADR-041 / FT-073).
    The shared escape hatches ``emit_triple`` / ``link_to`` /
    ``mark_boundary_artifact`` come from :class:`BuilderBase`.
    """

    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI
    TARGET_CLASS_LOCAL: str = 'QueryTemplate'
    SOURCE_SHAPE: str = 'workers/_shared/shapes/query-template.ttl'
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
    )

    P_querySpec: str = 'https://decision-cli.dev/ns#querySpec'
    P_version: str = 'https://decision-cli.dev/ns#version'
    P_queryLanguage: str = 'https://decision-cli.dev/ns#queryLanguage'
    P_addresses: str = 'https://decision-cli.dev/ns#addresses'
    P_decomposesFrom: str = 'https://decision-cli.dev/ns#decomposesFrom'

    def __init__(self, iri: str) -> None:
        super().__init__(iri)
        self._querySpec: str | None = None
        self._version: str | None = None
        self._queryLanguage: str | None = None
        self._addresses: list[str] = []
        self._decomposesFrom: list[str] = []

    def set_querySpec(self, value: str) -> "QueryTemplateBuilder":
        """Set the required body field ``querySpec``."""
        self._querySpec = value
        return self

    def set_version(self, value: str) -> "QueryTemplateBuilder":
        """Set the required body field ``version``."""
        self._version = value
        return self

    def set_queryLanguage(self, target_iri: str) -> "QueryTemplateBuilder":
        """Set forward edge ``queryLanguage``."""
        self._queryLanguage = target_iri
        return self

    def add_addresses(self, target_iri: str) -> "QueryTemplateBuilder":
        """Add a motivational ``addresses`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._addresses.append(target_iri)
        return self

    def add_decomposesFrom(self, target_iri: str) -> "QueryTemplateBuilder":
        """Add a motivational ``decomposesFrom`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._decomposesFrom.append(target_iri)
        return self

    def _validate_required(self) -> None:
        """Per-shape body-field cardinality check (FT-080 / ADR-041)."""
        if self._querySpec is None:
            from .._base import CommitError
            raise CommitError(
                self.TARGET_CLASS_LOCAL,
                "missing required body field: dec:querySpec",
                focus_iri=self.iri,
            )
        if self._version is None:
            from .._base import CommitError
            raise CommitError(
                self.TARGET_CLASS_LOCAL,
                "missing required body field: dec:version",
                focus_iri=self.iri,
            )

    def _motivational_state(self) -> dict[str, bool]:
        """Map ``{predicate_local: any_added}`` for SHACL ``sh:or`` evaluation."""
        return {
            'addresses': bool(self._addresses),
            'decomposesFrom': bool(self._decomposesFrom),
        }

    def _type_triples(self) -> list[tuple[str, str, str]]:
        """rdf:type + per-shape body triples (used by ``commit``)."""
        triples: list[tuple[str, str, str]] = []
        triples.append((self.iri, RDF_TYPE_IRI, self.TARGET_CLASS_IRI))
        if self._querySpec is not None:
            triples.append((self.iri, self.P_querySpec, str(self._querySpec)))
        if self._version is not None:
            triples.append((self.iri, self.P_version, str(self._version)))
        if self._queryLanguage is not None:
            triples.append((self.iri, self.P_queryLanguage, self._queryLanguage))
        for v in self._addresses:
            triples.append((self.iri, self.P_addresses, v))
        for v in self._decomposesFrom:
            triples.append((self.iri, self.P_decomposesFrom, v))
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
    "QueryTemplateBuilder",
    "TARGET_CLASS_IRI",
]
