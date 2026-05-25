"""Typed builder for dec:ADR artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/adr.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterable


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#ADR'
RDF_TYPE_IRI = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"


class ADRBuilder:
    """Builder for emitting dec:ADR artifacts.

    Workers call ``add_*`` / ``set_*`` then ``to_triples()`` to obtain
    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.
    SHACL conformance is re-validated authoritatively on the harness side
    (ADR-041); this builder enforces only the per-field cardinality the
    SHACL shape declares, as a fast-feedback check.
    """

    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI

    P_addresses: str = 'https://decision-cli.dev/ns#addresses'
    P_decidesFor: str = 'https://decision-cli.dev/ns#decidesFor'
    P_supersedes: str = 'https://decision-cli.dev/ns#supersedes'

    def __init__(self, iri: str) -> None:
        if not iri:
            raise ValueError("artifact IRI must not be empty")
        self.iri: str = iri
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
        for v in self._decidesFor:
            triples.append((self.iri, self.P_decidesFor, v))
        for v in self._supersedes:
            triples.append((self.iri, self.P_supersedes, v))
        return triples


__all__ = [
    "ADRBuilder",
    "TARGET_CLASS_IRI",
]
