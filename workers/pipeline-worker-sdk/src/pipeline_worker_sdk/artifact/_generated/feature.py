"""Typed builder for dec:Feature artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/feature.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterable


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Feature'
RDF_TYPE_IRI = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"


class FeatureBuilder:
    """Builder for emitting dec:Feature artifacts.

    Workers call ``add_*`` / ``set_*`` then ``to_triples()`` to obtain
    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.
    SHACL conformance is re-validated authoritatively on the harness side
    (ADR-041); this builder enforces only the per-field cardinality the
    SHACL shape declares, as a fast-feedback check.
    """

    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI

    P_addresses: str = 'https://decision-cli.dev/ns#addresses'
    P_decomposesFrom: str = 'https://decision-cli.dev/ns#decomposesFrom'
    P_originatedFrom: str = 'https://decision-cli.dev/ns#originatedFrom'
    P_respondsTo: str = 'https://decision-cli.dev/ns#respondsTo'

    def __init__(self, iri: str) -> None:
        if not iri:
            raise ValueError("artifact IRI must not be empty")
        self.iri: str = iri
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
        """Lightweight required-field check; SHACL is authoritative."""
        return None

    def to_triples(self) -> list[tuple[str, str, str]]:
        """Return ``(subject, predicate, object)`` triples for this artifact.

        Objects are returned as strings: IRIs for edges, lexical forms for
        body-field values. The caller is responsible for quoting / datatype
        annotation when serializing to N-Quads.
        """
        self._validate_required()
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


__all__ = [
    "FeatureBuilder",
    "TARGET_CLASS_IRI",
]
