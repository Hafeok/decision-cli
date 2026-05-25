"""Typed builder for dec:DiscoveryFinding artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/discovery-finding.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterable


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#DiscoveryFinding'
RDF_TYPE_IRI = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"


class DiscoveryFindingBuilder:
    """Builder for emitting dec:DiscoveryFinding artifacts.

    Workers call ``add_*`` / ``set_*`` then ``to_triples()`` to obtain
    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.
    SHACL conformance is re-validated authoritatively on the harness side
    (ADR-041); this builder enforces only the per-field cardinality the
    SHACL shape declares, as a fast-feedback check.
    """

    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI

    P_derivedFrom: str = 'https://decision-cli.dev/ns#derivedFrom'

    def __init__(self, iri: str) -> None:
        if not iri:
            raise ValueError("artifact IRI must not be empty")
        self.iri: str = iri
        self._derivedFrom: list[str] = []

    def add_derivedFrom(self, target_iri: str) -> "DiscoveryFindingBuilder":
        """Add a motivational ``derivedFrom`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._derivedFrom.append(target_iri)
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
        for v in self._derivedFrom:
            triples.append((self.iri, self.P_derivedFrom, v))
        return triples


__all__ = [
    "DiscoveryFindingBuilder",
    "TARGET_CLASS_IRI",
]
