"""Typed builder for dec:Feedback artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/feedback.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterable


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Feedback'
RDF_TYPE_IRI = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"


class FeedbackBuilder:
    """Builder for emitting dec:Feedback artifacts.

    Workers call ``add_*`` / ``set_*`` then ``to_triples()`` to obtain
    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.
    SHACL conformance is re-validated authoritatively on the harness side
    (ADR-041); this builder enforces only the per-field cardinality the
    SHACL shape declares, as a fast-feedback check.
    """

    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI

    P_observedIn: str = 'https://decision-cli.dev/ns#observedIn'
    P_observedVia: str = 'https://decision-cli.dev/ns#observedVia'
    P_producedBy: str = 'https://decision-cli.dev/ns#producedBy'

    def __init__(self, iri: str) -> None:
        if not iri:
            raise ValueError("artifact IRI must not be empty")
        self.iri: str = iri
        self._observedIn: list[str] = []
        self._observedVia: list[str] = []
        self._producedBy: list[str] = []

    def add_observedIn(self, target_iri: str) -> "FeedbackBuilder":
        """Add a motivational ``observedIn`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._observedIn.append(target_iri)
        return self

    def add_observedVia(self, target_iri: str) -> "FeedbackBuilder":
        """Add a motivational ``observedVia`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._observedVia.append(target_iri)
        return self

    def add_producedBy(self, target_iri: str) -> "FeedbackBuilder":
        """Add a motivational ``producedBy`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._producedBy.append(target_iri)
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
        for v in self._observedIn:
            triples.append((self.iri, self.P_observedIn, v))
        for v in self._observedVia:
            triples.append((self.iri, self.P_observedVia, v))
        for v in self._producedBy:
            triples.append((self.iri, self.P_producedBy, v))
        return triples


__all__ = [
    "FeedbackBuilder",
    "TARGET_CLASS_IRI",
]
