"""Typed builder for dec:Question artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/question.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterable


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Question'
RDF_TYPE_IRI = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"


class QuestionBuilder:
    """Builder for emitting dec:Question artifacts.

    Workers call ``add_*`` / ``set_*`` then ``to_triples()`` to obtain
    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.
    SHACL conformance is re-validated authoritatively on the harness side
    (ADR-041); this builder enforces only the per-field cardinality the
    SHACL shape declares, as a fast-feedback check.
    """

    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI

    P_raisedBy: str = 'https://decision-cli.dev/ns#raisedBy'
    P_raisedIn: str = 'https://decision-cli.dev/ns#raisedIn'

    def __init__(self, iri: str) -> None:
        if not iri:
            raise ValueError("artifact IRI must not be empty")
        self.iri: str = iri
        self._raisedBy: list[str] = []
        self._raisedIn: list[str] = []

    def add_raisedBy(self, target_iri: str) -> "QuestionBuilder":
        """Add a motivational ``raisedBy`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._raisedBy.append(target_iri)
        return self

    def add_raisedIn(self, target_iri: str) -> "QuestionBuilder":
        """Add a motivational ``raisedIn`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._raisedIn.append(target_iri)
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
        for v in self._raisedBy:
            triples.append((self.iri, self.P_raisedBy, v))
        for v in self._raisedIn:
            triples.append((self.iri, self.P_raisedIn, v))
        return triples


__all__ = [
    "QuestionBuilder",
    "TARGET_CLASS_IRI",
]
