"""Artifact-builder emitter — write-side typed surface (FT-085 / ADR-048)."""

from __future__ import annotations

from .emit_common import (
    const_safe,
    finalize,
    header_lines,
    is_already_emitted,
    safe_attr,
)
from .shapes import ShapeSpec


def emit_artifact_builder(spec: ShapeSpec) -> str:
    cls = spec.target_class_local
    motivational_preds = sorted({m.predicate_local for m in spec.motivational})
    lines: list[str] = []
    lines.extend(header_lines(spec, f"Typed builder for dec:{cls} artifacts."))
    lines.append("from __future__ import annotations")
    lines.append("")
    lines.append("from dataclasses import dataclass, field")
    lines.append("from typing import Iterable")
    lines.append("")
    lines.append("")
    lines.append(f"TARGET_CLASS_IRI = {spec.target_class_iri!r}")
    lines.append(
        'RDF_TYPE_IRI = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"'
    )
    lines.append("")
    lines.append("")
    lines.append(f"class {cls}Builder:")
    lines.append(_class_docstring(cls))
    lines.append("")
    lines.append("    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI")
    lines.append("")
    _emit_constants(lines, spec)
    lines.append("")
    _emit_init(lines, spec, motivational_preds)
    lines.append("")
    _emit_setters(lines, spec, motivational_preds)
    _emit_validator(lines, spec)
    lines.append("")
    _emit_to_triples(lines, spec, motivational_preds)
    lines.append("")
    lines.append("")
    lines.append("__all__ = [")
    lines.append(f'    "{cls}Builder",')
    lines.append('    "TARGET_CLASS_IRI",')
    lines.append("]")
    return finalize(lines)


def _class_docstring(cls: str) -> str:
    return (
        f'    """Builder for emitting dec:{cls} artifacts.\n'
        f"\n"
        f"    Workers call ``add_*`` / ``set_*`` then ``to_triples()`` to obtain\n"
        f"    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.\n"
        f"    SHACL conformance is re-validated authoritatively on the harness side\n"
        f"    (ADR-041); this builder enforces only the per-field cardinality the\n"
        f"    SHACL shape declares, as a fast-feedback check.\n"
        f'    """'
    )


def _emit_constants(lines: list[str], spec: ShapeSpec) -> None:
    emitted: set[str] = set()
    for f in spec.fields:
        const = f"P_{const_safe(f.local_name)}"
        if const in emitted:
            continue
        emitted.add(const)
        lines.append(f"    {const}: str = {f.iri!r}")
    for e in spec.edges:
        const = f"P_{const_safe(e.local_name)}"
        if const in emitted:
            continue
        emitted.add(const)
        lines.append(f"    {const}: str = {e.iri!r}")
    for m in spec.motivational:
        const = f"P_{const_safe(m.predicate_local)}"
        if const in emitted:
            continue
        emitted.add(const)
        lines.append(f"    {const}: str = {m.predicate_iri!r}")


def _emit_init(
    lines: list[str], spec: ShapeSpec, motivational_preds: list[str]
) -> None:
    lines.append("    def __init__(self, iri: str) -> None:")
    lines.append('        if not iri:')
    lines.append('            raise ValueError("artifact IRI must not be empty")')
    lines.append("        self.iri: str = iri")
    for f in spec.fields:
        attr = safe_attr(f.local_name)
        if f.single_valued:
            lines.append(f"        self._{attr}: {f.python_type} | None = None")
        else:
            lines.append(f"        self._{attr}: list[{f.python_type}] = []")
    for e in spec.edges:
        attr = safe_attr(e.local_name)
        if e.single_valued:
            lines.append(f"        self._{attr}: str | None = None")
        else:
            lines.append(f"        self._{attr}: list[str] = []")
    for pred in motivational_preds:
        if is_already_emitted(pred, spec):
            continue
        lines.append(f"        self._{safe_attr(pred)}: list[str] = []")


def _emit_setters(
    lines: list[str], spec: ShapeSpec, motivational_preds: list[str]
) -> None:
    cls = spec.target_class_local
    for f in spec.fields:
        attr = safe_attr(f.local_name)
        if f.single_valued:
            lines.append(
                f"    def set_{attr}(self, value: {f.python_type}) "
                f'-> "{cls}Builder":'
            )
            lines.append(
                f'        """Set the required body field ``{f.local_name}``."""'
            )
            lines.append(f"        self._{attr} = value")
            lines.append("        return self")
            lines.append("")
        else:
            lines.append(
                f"    def add_{attr}(self, value: {f.python_type}) "
                f'-> "{cls}Builder":'
            )
            lines.append(f"        self._{attr}.append(value)")
            lines.append("        return self")
            lines.append("")
    for e in spec.edges:
        attr = safe_attr(e.local_name)
        verb = "set" if e.single_valued else "add"
        lines.append(
            f'    def {verb}_{attr}(self, target_iri: str) -> "{cls}Builder":'
        )
        lines.append(
            f'        """{verb.capitalize()} forward edge ``{e.local_name}``."""'
        )
        if e.single_valued:
            lines.append(f"        self._{attr} = target_iri")
        else:
            lines.append(f"        self._{attr}.append(target_iri)")
        lines.append("        return self")
        lines.append("")
    for pred in motivational_preds:
        if is_already_emitted(pred, spec):
            continue
        attr = safe_attr(pred)
        lines.append(
            f'    def add_{attr}(self, target_iri: str) -> "{cls}Builder":'
        )
        lines.append(
            f'        """Add a motivational ``{pred}`` edge (one of the\n'
            f"        sh:or alternatives declared in the per-type shape).\n"
            f'        """'
        )
        lines.append(f"        self._{attr}.append(target_iri)")
        lines.append("        return self")
        lines.append("")


def _emit_validator(lines: list[str], spec: ShapeSpec) -> None:
    lines.append("    def _validate_required(self) -> None:")
    lines.append(
        '        """Lightweight required-field check; SHACL is authoritative."""'
    )
    any_required = False
    for f in spec.fields:
        if not f.required:
            continue
        any_required = True
        attr = safe_attr(f.local_name)
        cond = f"self._{attr} is None" if f.single_valued else f"not self._{attr}"
        lines.append(f"        if {cond}:")
        lines.append(
            f"            raise ValueError("
            f'"missing required body field: {f.local_name}")'
        )
    if not any_required:
        lines.append("        return None")


def _emit_to_triples(
    lines: list[str], spec: ShapeSpec, motivational_preds: list[str]
) -> None:
    lines.append("    def to_triples(self) -> list[tuple[str, str, str]]:")
    lines.append(
        '        """Return ``(subject, predicate, object)`` triples for this artifact.\n'
        "\n"
        "        Objects are returned as strings: IRIs for edges, lexical forms for\n"
        "        body-field values. The caller is responsible for quoting / datatype\n"
        "        annotation when serializing to N-Quads.\n"
        '        """'
    )
    lines.append("        self._validate_required()")
    lines.append("        triples: list[tuple[str, str, str]] = []")
    lines.append(
        "        triples.append((self.iri, RDF_TYPE_IRI, self.TARGET_CLASS_IRI))"
    )
    for f in spec.fields:
        attr = safe_attr(f.local_name)
        const = f"self.P_{const_safe(f.local_name)}"
        if f.single_valued:
            lines.append(f"        if self._{attr} is not None:")
            lines.append(
                f"            triples.append((self.iri, {const}, str(self._{attr})))"
            )
        else:
            lines.append(f"        for v in self._{attr}:")
            lines.append(
                f"            triples.append((self.iri, {const}, str(v)))"
            )
    for e in spec.edges:
        attr = safe_attr(e.local_name)
        const = f"self.P_{const_safe(e.local_name)}"
        if e.single_valued:
            lines.append(f"        if self._{attr} is not None:")
            lines.append(
                f"            triples.append((self.iri, {const}, self._{attr}))"
            )
        else:
            lines.append(f"        for v in self._{attr}:")
            lines.append(f"            triples.append((self.iri, {const}, v))")
    for pred in motivational_preds:
        if is_already_emitted(pred, spec):
            continue
        attr = safe_attr(pred)
        const = f"self.P_{const_safe(pred)}"
        lines.append(f"        for v in self._{attr}:")
        lines.append(f"            triples.append((self.iri, {const}, v))")
    lines.append("        return triples")
