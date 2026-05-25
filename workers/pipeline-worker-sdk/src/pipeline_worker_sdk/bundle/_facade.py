"""Hand-written Bundle facade exposing curated query helpers (FT-079 / ADR-048)."""

from __future__ import annotations

import dataclasses
from collections.abc import Callable, Iterable
from importlib import import_module
from typing import Any

import pyoxigraph

from . import _generated as accessors

DEC_NS = "https://decision-cli.dev/ns#"
RDF_TYPE_IRI = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
ADR_TYPE_IRI = f"{DEC_NS}ADR"
TC_TYPE_IRI = f"{DEC_NS}TC"


def _build_class_registry() -> dict[str, type]:
    """Map ``TARGET_CLASS_IRI`` → ``…Accessor`` class for every generated type."""
    registry: dict[str, type] = {}
    for name in accessors.__all__:
        if not name.endswith("Accessor"):
            continue
        cls = getattr(accessors, name)
        module = import_module(cls.__module__)
        iri = getattr(module, "TARGET_CLASS_IRI", None)
        if iri:
            registry[iri] = cls
    return registry


_CLASS_REGISTRY: dict[str, type] = _build_class_registry()


class UnknownFocalTypeError(LookupError):
    """Raised when the focal IRI's rdf:type has no generated accessor."""


def _named(iri: str) -> pyoxigraph.NamedNode:
    return pyoxigraph.NamedNode(iri)


def _types_of(store: pyoxigraph.Store, iri: str) -> tuple[str, ...]:
    """Return every ``rdf:type`` value declared on ``iri`` in deterministic order."""
    subj = _named(iri)
    pred = _named(RDF_TYPE_IRI)
    values: list[str] = []
    for quad in store.quads_for_pattern(subj, pred, None, None):
        obj = quad.object
        if isinstance(obj, pyoxigraph.NamedNode):
            values.append(obj.value)
    return tuple(sorted(set(values)))


def _values_for(store: pyoxigraph.Store, iri: str, predicate: str) -> list[Any]:
    """Return literal/IRI values for ``iri predicate ?o`` in sorted order."""
    subj = _named(iri)
    pred = _named(predicate)
    seen: set[tuple[str, str]] = set()
    out: list[Any] = []
    for quad in store.quads_for_pattern(subj, pred, None, None):
        obj = quad.object
        if isinstance(obj, pyoxigraph.NamedNode):
            key = ("iri", obj.value)
            if key in seen:
                continue
            seen.add(key)
            out.append(obj.value)
        elif isinstance(obj, pyoxigraph.Literal):
            key = ("lit", obj.value)
            if key in seen:
                continue
            seen.add(key)
            out.append(obj.value)
        # BlankNode and other terms are ignored: the bundle surface only
        # exposes named edges/literals; blank-node lineage is gap territory.
    out.sort(key=lambda v: ("" if v is None else str(v)))
    return out


def _is_multi_valued(field: dataclasses.Field) -> bool:
    """Detect ``tuple[..., ...]`` annotations on a dataclass field.

    With ``from __future__ import annotations`` in the generated modules the
    field's ``type`` attribute is a string. We pattern-match on that string
    rather than evaluating the annotation, which keeps the facade free of
    forward-reference resolution concerns.
    """
    annotation = field.type
    if not isinstance(annotation, str):
        annotation = str(annotation)
    return annotation.lstrip().startswith("tuple[")


def _build_accessor(cls: type, iri: str, store: pyoxigraph.Store) -> Any:
    """Construct ``cls`` by populating each declared field from ``store``."""
    init_kwargs: dict[str, Any] = {"iri": iri}
    for field in dataclasses.fields(cls):
        if field.name == "iri":
            continue
        predicate = f"{DEC_NS}{field.name}"
        raw_values = _values_for(store, iri, predicate)
        if _is_multi_valued(field):
            init_kwargs[field.name] = tuple(raw_values)
        else:
            init_kwargs[field.name] = raw_values[0] if raw_values else None
    return cls(**init_kwargs)


def _subjects_with_type(
    store: pyoxigraph.Store, type_iri: str
) -> tuple[str, ...]:
    """All IRIs in ``store`` declared as ``rdf:type type_iri`` in sorted order."""
    pred = _named(RDF_TYPE_IRI)
    obj = _named(type_iri)
    out: set[str] = set()
    for quad in store.quads_for_pattern(None, pred, obj, None):
        if isinstance(quad.subject, pyoxigraph.NamedNode):
            out.add(quad.subject.value)
    return tuple(sorted(out))


def _subjects_pointing_at(
    store: pyoxigraph.Store, predicate: str, target_iri: str
) -> tuple[str, ...]:
    """All IRIs ``?s`` such that ``?s predicate target_iri`` in sorted order."""
    pred = _named(predicate)
    target = _named(target_iri)
    out: set[str] = set()
    for quad in store.quads_for_pattern(None, pred, target, None):
        if isinstance(quad.subject, pyoxigraph.NamedNode):
            out.add(quad.subject.value)
    return tuple(sorted(out))


class Bundle:
    """Curated read-only surface over a session's in-memory bundle sub-graph.

    Workers consume the bundle through this facade rather than writing SPARQL
    by hand (ADR-048). Each accessor is deterministic: same store + same
    focal IRI ⇒ identical return values across calls and across worker
    processes. The raw store remains reachable via :attr:`raw_store`, but
    every access trips an instrumentation callback so the harness can surface
    gap-surface patterns in completion telemetry (FT-079 success criteria).
    """

    def __init__(
        self,
        store: pyoxigraph.Store,
        focal_iri: str,
        *,
        on_raw_store_access: Callable[[], None] | None = None,
    ) -> None:
        self._store = store
        self._focal_iri = focal_iri
        self._on_raw_store_access = on_raw_store_access
        self._raw_store_access_count = 0

    # ------------------------------------------------------------------ #
    # Identity                                                           #
    # ------------------------------------------------------------------ #

    @property
    def focal_iri(self) -> str:
        """The IRI of the artifact under work."""
        return self._focal_iri

    @property
    def raw_store_access_count(self) -> int:
        """Cumulative count of :attr:`raw_store` accesses through this facade."""
        return self._raw_store_access_count

    # ------------------------------------------------------------------ #
    # Curated accessors                                                  #
    # ------------------------------------------------------------------ #

    def focal(self) -> Any:
        """Typed accessor for the focal artifact.

        Resolves the focal IRI's ``rdf:type`` against the codegen registry and
        returns the matching ``…Accessor`` instance. Raises
        :class:`UnknownFocalTypeError` if no generated accessor covers the
        focal's type — the caller's next-best move is ``raw_store`` plus a
        gap-surface signal so the codegen catalog can be extended.
        """
        type_iris = _types_of(self._store, self._focal_iri)
        for type_iri in type_iris:
            cls = _CLASS_REGISTRY.get(type_iri)
            if cls is not None:
                return _build_accessor(cls, self._focal_iri, self._store)
        raise UnknownFocalTypeError(
            f"no generated accessor for focal {self._focal_iri!r}; "
            f"declared rdf:type values: {list(type_iris)!r}"
        )

    def linked_adrs(self) -> tuple[Any, ...]:
        """ADRs in the bundle that govern the focal artifact.

        A bundle ADR is considered linked when its ``dec:decidesFor`` edge
        targets the focal IRI — that is the bundle SHACL's declared
        motivational predicate from an ADR to a Feature (per ADR-038's
        ``adr.ttl``). The return order is lexicographic on IRI so two
        workers reading the same store get byte-identical tuples.
        """
        cls = _CLASS_REGISTRY[ADR_TYPE_IRI]
        return _accessors_for(
            store=self._store,
            cls=cls,
            iris=_subjects_pointing_at(
                self._store,
                predicate=f"{DEC_NS}decidesFor",
                target_iri=self._focal_iri,
            ),
        )

    def applicable_test_criteria(self) -> tuple[Any, ...]:
        """TCs whose ``dec:validates`` edge targets the focal artifact.

        These are the test criteria the action must satisfy for the focal
        artifact's verify to pass. The return order is lexicographic on IRI
        for cross-worker determinism.
        """
        cls = _CLASS_REGISTRY[TC_TYPE_IRI]
        return _accessors_for(
            store=self._store,
            cls=cls,
            iris=_subjects_pointing_at(
                self._store,
                predicate=f"{DEC_NS}validates",
                target_iri=self._focal_iri,
            ),
        )

    def accessors_of_type(self, type_iri: str) -> tuple[Any, ...]:
        """All accessors of the given target class in the bundle, sorted by IRI.

        Generic role-specific surface for accessors not in the curated set;
        prefer the named methods above when applicable. Raises
        :class:`UnknownFocalTypeError` for unregistered types so callers do
        not silently get an empty tuple.
        """
        cls = _CLASS_REGISTRY.get(type_iri)
        if cls is None:
            raise UnknownFocalTypeError(
                f"no generated accessor registered for type {type_iri!r}"
            )
        return _accessors_for(
            store=self._store,
            cls=cls,
            iris=_subjects_with_type(self._store, type_iri),
        )

    # ------------------------------------------------------------------ #
    # Escape hatch                                                       #
    # ------------------------------------------------------------------ #

    @property
    def raw_store(self) -> pyoxigraph.Store:
        """Direct access to the underlying pyoxigraph store.

        Each access increments :attr:`raw_store_access_count` and invokes the
        ``on_raw_store_access`` callback (typically wired by
        :meth:`Session.bundle` to bump a session telemetry counter that
        surfaces on the completion event). Repeated raw-store reads are a
        gap-surface signal: the curated facade does not cover whatever the
        worker is reaching for, and codegen extension is the long-term fix.
        """
        self._raw_store_access_count += 1
        if self._on_raw_store_access is not None:
            self._on_raw_store_access()
        return self._store


def _accessors_for(
    *,
    store: pyoxigraph.Store,
    cls: type,
    iris: Iterable[str],
) -> tuple[Any, ...]:
    """Build a tuple of accessors for ``iris`` in caller-provided order."""
    return tuple(_build_accessor(cls, iri, store) for iri in iris)


__all__ = [
    "ADR_TYPE_IRI",
    "Bundle",
    "DEC_NS",
    "TC_TYPE_IRI",
    "UnknownFocalTypeError",
]
