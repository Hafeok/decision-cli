"""Typed builder for dec:Brief artifacts."""

# ============================================================
# GENERATED FILE — DO NOT EDIT BY HAND.
# Regenerate via:  uv run codegen
# ============================================================
# Source SHACL shape: workers/_shared/shapes/brief.ttl
# Generator: pipeline-worker-sdk codegen (FT-085 / ADR-048)

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterable


TARGET_CLASS_IRI = 'https://decision-cli.dev/ns#Brief'
RDF_TYPE_IRI = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"


class BriefBuilder:
    """Builder for emitting dec:Brief artifacts.

    Workers call ``add_*`` / ``set_*`` then ``to_triples()`` to obtain
    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.
    SHACL conformance is re-validated authoritatively on the harness side
    (ADR-041); this builder enforces only the per-field cardinality the
    SHACL shape declares, as a fast-feedback check.
    """

    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI

    P_goal: str = 'https://decision-cli.dev/ns#goal'
    P_premise: str = 'https://decision-cli.dev/ns#premise'
    P_successCriteria: str = 'https://decision-cli.dev/ns#successCriteria'
    P_title: str = 'https://decision-cli.dev/ns#title'
    P_acknowledges: str = 'https://decision-cli.dev/ns#acknowledges'
    P_decomposesInto: str = 'https://decision-cli.dev/ns#decomposesInto'
    P_excludes: str = 'https://decision-cli.dev/ns#excludes'
    P_references: str = 'https://decision-cli.dev/ns#references'
    P_respondsTo: str = 'https://decision-cli.dev/ns#respondsTo'

    def __init__(self, iri: str) -> None:
        if not iri:
            raise ValueError("artifact IRI must not be empty")
        self.iri: str = iri
        self._goal: str | None = None
        self._premise: str | None = None
        self._successCriteria: str | None = None
        self._title: str | None = None
        self._acknowledges: list[str] = []
        self._decomposesInto: list[str] = []
        self._excludes: list[str] = []
        self._references: list[str] = []
        self._respondsTo: list[str] = []

    def set_goal(self, value: str) -> "BriefBuilder":
        """Set the required body field ``goal``."""
        self._goal = value
        return self

    def set_premise(self, value: str) -> "BriefBuilder":
        """Set the required body field ``premise``."""
        self._premise = value
        return self

    def set_successCriteria(self, value: str) -> "BriefBuilder":
        """Set the required body field ``successCriteria``."""
        self._successCriteria = value
        return self

    def set_title(self, value: str) -> "BriefBuilder":
        """Set the required body field ``title``."""
        self._title = value
        return self

    def add_acknowledges(self, target_iri: str) -> "BriefBuilder":
        """Add forward edge ``acknowledges``."""
        self._acknowledges.append(target_iri)
        return self

    def add_decomposesInto(self, target_iri: str) -> "BriefBuilder":
        """Add forward edge ``decomposesInto``."""
        self._decomposesInto.append(target_iri)
        return self

    def add_excludes(self, target_iri: str) -> "BriefBuilder":
        """Add forward edge ``excludes``."""
        self._excludes.append(target_iri)
        return self

    def add_references(self, target_iri: str) -> "BriefBuilder":
        """Add forward edge ``references``."""
        self._references.append(target_iri)
        return self

    def add_respondsTo(self, target_iri: str) -> "BriefBuilder":
        """Add a motivational ``respondsTo`` edge (one of the
        sh:or alternatives declared in the per-type shape).
        """
        self._respondsTo.append(target_iri)
        return self

    def _validate_required(self) -> None:
        """Lightweight required-field check; SHACL is authoritative."""
        if self._goal is None:
            raise ValueError("missing required body field: goal")
        if self._premise is None:
            raise ValueError("missing required body field: premise")
        if self._successCriteria is None:
            raise ValueError("missing required body field: successCriteria")
        if self._title is None:
            raise ValueError("missing required body field: title")

    def to_triples(self) -> list[tuple[str, str, str]]:
        """Return ``(subject, predicate, object)`` triples for this artifact.

        Objects are returned as strings: IRIs for edges, lexical forms for
        body-field values. The caller is responsible for quoting / datatype
        annotation when serializing to N-Quads.
        """
        self._validate_required()
        triples: list[tuple[str, str, str]] = []
        triples.append((self.iri, RDF_TYPE_IRI, self.TARGET_CLASS_IRI))
        if self._goal is not None:
            triples.append((self.iri, self.P_goal, str(self._goal)))
        if self._premise is not None:
            triples.append((self.iri, self.P_premise, str(self._premise)))
        if self._successCriteria is not None:
            triples.append((self.iri, self.P_successCriteria, str(self._successCriteria)))
        if self._title is not None:
            triples.append((self.iri, self.P_title, str(self._title)))
        for v in self._acknowledges:
            triples.append((self.iri, self.P_acknowledges, v))
        for v in self._decomposesInto:
            triples.append((self.iri, self.P_decomposesInto, v))
        for v in self._excludes:
            triples.append((self.iri, self.P_excludes, v))
        for v in self._references:
            triples.append((self.iri, self.P_references, v))
        for v in self._respondsTo:
            triples.append((self.iri, self.P_respondsTo, v))
        return triples


__all__ = [
    "BriefBuilder",
    "TARGET_CLASS_IRI",
]
