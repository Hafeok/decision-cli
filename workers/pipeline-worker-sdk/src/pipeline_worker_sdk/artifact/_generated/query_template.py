"""Typed builder for dec:QueryTemplate artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/query-template.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterable


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#QueryTemplate'
RDF_TYPE_IRI = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"


class QueryTemplateBuilder:
    """Builder for emitting dec:QueryTemplate artifacts.

    Workers call ``add_*`` / ``set_*`` then ``to_triples()`` to obtain
    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.
    SHACL conformance is re-validated authoritatively on the harness side
    (ADR-041); this builder enforces only the per-field cardinality the
    SHACL shape declares, as a fast-feedback check.
    """

    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI

    P_querySpec: str = 'https://decision-cli.dev/ns#querySpec'
    P_version: str = 'https://decision-cli.dev/ns#version'
    P_queryLanguage: str = 'https://decision-cli.dev/ns#queryLanguage'
    P_addresses: str = 'https://decision-cli.dev/ns#addresses'
    P_decomposesFrom: str = 'https://decision-cli.dev/ns#decomposesFrom'

    def __init__(self, iri: str) -> None:
        if not iri:
            raise ValueError("artifact IRI must not be empty")
        self.iri: str = iri
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
        """Lightweight required-field check; SHACL is authoritative."""
        if self._querySpec is None:
            raise ValueError("missing required body field: querySpec")
        if self._version is None:
            raise ValueError("missing required body field: version")

    def to_triples(self) -> list[tuple[str, str, str]]:
        """Return ``(subject, predicate, object)`` triples for this artifact.

        Objects are returned as strings: IRIs for edges, lexical forms for
        body-field values. The caller is responsible for quoting / datatype
        annotation when serializing to N-Quads.
        """
        self._validate_required()
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


__all__ = [
    "QueryTemplateBuilder",
    "TARGET_CLASS_IRI",
]
