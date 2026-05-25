"""SHACL-graph parsing internals (FT-085 / ADR-048).

Walks the parsed RDF graph produced by ``pyoxigraph`` and extracts the
per-shape metadata the emitters consume. Public entry point is
:func:`extract_specs_for_file`; everything else is implementation detail
the loader in :mod:`shapes` reaches through.
"""

from __future__ import annotations

from pathlib import Path
from typing import Iterable, List, Optional, Set

import pyoxigraph

from .shapes_types import (
    BOUNDARY_ARTIFACT,
    DATATYPE_TO_PYTHON,
    EdgeField,
    MotivationalAlternative,
    NS_DEC,
    PropertyField,
    RDF_FIRST,
    RDF_NIL,
    RDF_REST,
    SH_CLASS,
    SH_DATATYPE,
    SH_MAX_COUNT,
    SH_MIN_COUNT,
    SH_OR,
    SH_PATH,
    SH_PROPERTY,
    SH_TARGET_CLASS,
    ShapeSpec,
)


def extract_specs_for_file(path: Path) -> List[ShapeSpec]:
    store = pyoxigraph.Store()
    store.load(
        input=path.read_text(encoding="utf-8"),
        format=pyoxigraph.RdfFormat.TURTLE,
    )
    shape_iris = _shapes_with_target_class(store)
    specs: List[ShapeSpec] = []
    for shape_iri, target_iri in shape_iris:
        if not target_iri.startswith(NS_DEC):
            continue
        local = target_iri[len(NS_DEC):]
        fields = tuple(
            sorted(_extract_fields(store, shape_iri), key=lambda f: f.local_name)
        )
        edges = tuple(
            sorted(_extract_edges(store, shape_iri), key=lambda e: e.local_name)
        )
        motivational, accepts_boundary = _extract_motivational(store, shape_iri)
        specs.append(
            ShapeSpec(
                source_file=path.name,
                shape_iri=shape_iri,
                target_class_iri=target_iri,
                target_class_local=local,
                fields=fields,
                edges=edges,
                motivational=tuple(
                    sorted(
                        motivational,
                        key=lambda m: (m.predicate_local, m.target_class_local),
                    )
                ),
                accepts_boundary=accepts_boundary,
            )
        )
    return specs


def _shapes_with_target_class(store: pyoxigraph.Store) -> List[tuple[str, str]]:
    out: list[tuple[str, str]] = []
    for quad in store.quads_for_pattern(
        None, pyoxigraph.NamedNode(SH_TARGET_CLASS), None, None
    ):
        subj = quad.subject
        obj = quad.object
        if isinstance(subj, pyoxigraph.NamedNode) and isinstance(
            obj, pyoxigraph.NamedNode
        ):
            out.append((subj.value, obj.value))
    return out


def _property_blocks(
    store: pyoxigraph.Store, shape_iri: str
) -> list[pyoxigraph.BlankNode]:
    out: list[pyoxigraph.BlankNode] = []
    for quad in store.quads_for_pattern(
        pyoxigraph.NamedNode(shape_iri),
        pyoxigraph.NamedNode(SH_PROPERTY),
        None,
        None,
    ):
        obj = quad.object
        if isinstance(obj, pyoxigraph.BlankNode):
            out.append(obj)
    return out


def _single_object(
    store: pyoxigraph.Store, subj: object, pred_iri: str
) -> Optional[object]:
    for quad in store.quads_for_pattern(
        subj, pyoxigraph.NamedNode(pred_iri), None, None
    ):
        return quad.object
    return None


def _extract_fields(
    store: pyoxigraph.Store, shape_iri: str
) -> List[PropertyField]:
    out: list[PropertyField] = []
    for blank in _property_blocks(store, shape_iri):
        path_obj = _single_object(store, blank, SH_PATH)
        dt_obj = _single_object(store, blank, SH_DATATYPE)
        if not isinstance(path_obj, pyoxigraph.NamedNode):
            continue
        if not isinstance(dt_obj, pyoxigraph.NamedNode):
            continue
        path_iri = path_obj.value
        if not path_iri.startswith(NS_DEC):
            continue
        py_type = DATATYPE_TO_PYTHON.get(dt_obj.value, "str")
        min_count = _literal_int(_single_object(store, blank, SH_MIN_COUNT))
        max_count = _literal_int(_single_object(store, blank, SH_MAX_COUNT))
        out.append(
            PropertyField(
                local_name=path_iri[len(NS_DEC):],
                iri=path_iri,
                datatype_iri=dt_obj.value,
                python_type=py_type,
                required=(min_count is not None and min_count >= 1),
                single_valued=(max_count is not None and max_count == 1),
            )
        )
    return out


def _extract_edges(store: pyoxigraph.Store, shape_iri: str) -> List[EdgeField]:
    out: list[EdgeField] = []
    for blank in _property_blocks(store, shape_iri):
        path_obj = _single_object(store, blank, SH_PATH)
        class_obj = _single_object(store, blank, SH_CLASS)
        dt_obj = _single_object(store, blank, SH_DATATYPE)
        if not isinstance(path_obj, pyoxigraph.NamedNode):
            continue
        if isinstance(dt_obj, pyoxigraph.NamedNode):
            continue
        path_iri = path_obj.value
        if not path_iri.startswith(NS_DEC):
            continue
        range_iri = (
            class_obj.value if isinstance(class_obj, pyoxigraph.NamedNode) else ""
        )
        range_local = (
            range_iri[len(NS_DEC):] if range_iri.startswith(NS_DEC) else range_iri
        )
        min_count = _literal_int(_single_object(store, blank, SH_MIN_COUNT))
        max_count = _literal_int(_single_object(store, blank, SH_MAX_COUNT))
        out.append(
            EdgeField(
                local_name=path_iri[len(NS_DEC):],
                iri=path_iri,
                range_class_iri=range_iri,
                range_class_local=range_local,
                required=(min_count is not None and min_count >= 1),
                single_valued=(max_count is not None and max_count == 1),
            )
        )
    return out


def _extract_motivational(
    store: pyoxigraph.Store, shape_iri: str
) -> tuple[List[MotivationalAlternative], bool]:
    out: list[MotivationalAlternative] = []
    accepts_boundary = False
    or_head = _single_object(store, pyoxigraph.NamedNode(shape_iri), SH_OR)
    if or_head is None:
        return out, accepts_boundary
    for branch in _iter_list_members(store, or_head):
        class_obj = _single_object(store, branch, SH_CLASS)
        if (
            isinstance(class_obj, pyoxigraph.NamedNode)
            and class_obj.value == BOUNDARY_ARTIFACT
        ):
            accepts_boundary = True
            continue
        prop_block = _single_object(store, branch, SH_PROPERTY)
        if prop_block is None:
            continue
        path_obj = _single_object(store, prop_block, SH_PATH)
        target_obj = _single_object(store, prop_block, SH_CLASS)
        if not isinstance(path_obj, pyoxigraph.NamedNode):
            continue
        if not isinstance(target_obj, pyoxigraph.NamedNode):
            continue
        path_iri = path_obj.value
        if not path_iri.startswith(NS_DEC):
            continue
        target_iri = target_obj.value
        target_local = (
            target_iri[len(NS_DEC):]
            if target_iri.startswith(NS_DEC)
            else target_iri
        )
        out.append(
            MotivationalAlternative(
                predicate_local=path_iri[len(NS_DEC):],
                predicate_iri=path_iri,
                target_class_iri=target_iri,
                target_class_local=target_local,
            )
        )
    return out, accepts_boundary


def _iter_list_members(
    store: pyoxigraph.Store, head: object
) -> Iterable[object]:
    cur = head
    seen: Set[str] = set()
    while True:
        if isinstance(cur, pyoxigraph.NamedNode) and cur.value == RDF_NIL:
            return
        key = _node_key(cur)
        if key in seen:
            return
        seen.add(key)
        first = _single_object(store, cur, RDF_FIRST)
        if first is None:
            return
        yield first
        rest = _single_object(store, cur, RDF_REST)
        if rest is None:
            return
        cur = rest


def _node_key(node: object) -> str:
    if isinstance(node, pyoxigraph.NamedNode):
        return f"n:{node.value}"
    if isinstance(node, pyoxigraph.BlankNode):
        return f"b:{node.value}"
    return repr(node)


def _literal_int(node: Optional[object]) -> Optional[int]:
    if isinstance(node, pyoxigraph.Literal):
        try:
            return int(node.value)
        except (TypeError, ValueError):
            return None
    return None
