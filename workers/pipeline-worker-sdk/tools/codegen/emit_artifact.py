"""Artifact-builder emitter — write-side typed surface (FT-085 / ADR-048 / FT-080)."""

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
    lines.append("from .._base import BuilderBase, MotivationalDescriptor, RDF_TYPE_IRI")
    lines.append("")
    lines.append("")
    lines.append(f"TARGET_CLASS_IRI = {spec.target_class_iri!r}")
    lines.append("")
    lines.append("")
    lines.append(f"class {cls}Builder(BuilderBase):")
    lines.append(_class_docstring(cls))
    lines.append("")
    lines.append(f"    TARGET_CLASS_IRI: str = TARGET_CLASS_IRI")
    lines.append(f"    TARGET_CLASS_LOCAL: str = {cls!r}")
    lines.append(f"    SOURCE_SHAPE: str = {f'workers/_shared/shapes/{spec.source_file}'!r}")
    lines.append(f"    ACCEPTS_BOUNDARY: bool = {spec.accepts_boundary!r}")
    _emit_motivational_class_var(lines, spec)
    lines.append("")
    _emit_constants(lines, spec)
    lines.append("")
    _emit_init(lines, spec, motivational_preds)
    lines.append("")
    _emit_setters(lines, spec, motivational_preds)
    _emit_required_validator(lines, spec)
    lines.append("")
    _emit_motivational_state(lines, spec, motivational_preds)
    lines.append("")
    _emit_type_triples(lines, spec, motivational_preds)
    lines.append("")
    _emit_legacy_to_triples(lines)
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
        f"    Workers call ``add_*`` / ``set_*`` then ``commit()`` to obtain\n"
        f"    a list of ``(s, p, o)`` triples ready for the harness's GraphWriter.\n"
        f"    SHACL conformance is enforced locally on ``commit()`` (FT-080) and\n"
        f"    re-validated authoritatively on the harness side (ADR-041 / FT-073).\n"
        f"    The shared escape hatches ``emit_triple`` / ``link_to`` /\n"
        f"    ``mark_boundary_artifact`` come from :class:`BuilderBase`.\n"
        f'    """'
    )


def _emit_motivational_class_var(lines: list[str], spec: ShapeSpec) -> None:
    if not spec.motivational:
        lines.append("    MOTIVATIONAL: tuple[MotivationalDescriptor, ...] = ()")
        return
    lines.append("    MOTIVATIONAL: tuple[MotivationalDescriptor, ...] = (")
    # Deduplicate by predicate-local (same shape can list one predicate twice
    # for different target classes — collapse into one descriptor labelling
    # both possibilities).
    seen: set[str] = set()
    for m in spec.motivational:
        if m.predicate_local in seen:
            continue
        seen.add(m.predicate_local)
        lines.append("        MotivationalDescriptor(")
        lines.append(f"            predicate_local={m.predicate_local!r},")
        lines.append(f"            predicate_iri={m.predicate_iri!r},")
        lines.append(f"            target_class_local={m.target_class_local!r},")
        lines.append(f"            target_class_iri={m.target_class_iri!r},")
        lines.append("        ),")
    lines.append("    )")


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
    lines.append("        super().__init__(iri)")
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


def _emit_required_validator(lines: list[str], spec: ShapeSpec) -> None:
    lines.append("    def _validate_required(self) -> None:")
    lines.append(
        '        """Per-shape body-field cardinality check (FT-080 / ADR-041)."""'
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
            f"            from .._base import CommitError"
        )
        lines.append(
            f'            raise CommitError(\n'
            f"                self.TARGET_CLASS_LOCAL,\n"
            f'                "missing required body field: dec:{f.local_name}",\n'
            f"                focus_iri=self.iri,\n"
            f"            )"
        )
    if not any_required:
        lines.append("        return None")


def _emit_motivational_state(
    lines: list[str], spec: ShapeSpec, motivational_preds: list[str]
) -> None:
    lines.append("    def _motivational_state(self) -> dict[str, bool]:")
    lines.append(
        '        """Map ``{predicate_local: any_added}`` for SHACL ``sh:or`` evaluation."""'
    )
    if not motivational_preds:
        lines.append("        return {}")
        return
    lines.append("        return {")
    for pred in motivational_preds:
        attr = safe_attr(pred)
        if is_already_emitted(pred, spec):
            # The predicate is also a regular body field / edge.
            # Determine if the matching field/edge is single- or multi-valued.
            single = _is_field_single_valued(spec, pred)
            cond = (
                f"self._{attr} is not None"
                if single
                else f"bool(self._{attr})"
            )
        else:
            cond = f"bool(self._{attr})"
        lines.append(f"            {pred!r}: {cond},")
    lines.append("        }")


def _is_field_single_valued(spec: ShapeSpec, pred_local: str) -> bool:
    for f in spec.fields:
        if f.local_name == pred_local:
            return f.single_valued
    for e in spec.edges:
        if e.local_name == pred_local:
            return e.single_valued
    return False


def _emit_type_triples(
    lines: list[str], spec: ShapeSpec, motivational_preds: list[str]
) -> None:
    lines.append("    def _type_triples(self) -> list[tuple[str, str, str]]:")
    lines.append(
        '        """rdf:type + per-shape body triples (used by ``commit``)."""'
    )
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


def _emit_legacy_to_triples(lines: list[str]) -> None:
    """Emit a back-compat ``to_triples()`` that mirrors pre-FT-080 callers."""
    lines.append("    def to_triples(self) -> list[tuple[str, str, str]]:")
    lines.append(
        '        """Backward-compatible accessor: returns the same triples\n'
        "        as :meth:`commit` without enforcing SHACL ``sh:or``.\n"
        "\n"
        "        New code should prefer :meth:`commit`, which raises on\n"
        "        missing motivational / required fields per FT-080 success\n"
        "        criterion 1.\n"
        '        """'
    )
    lines.append("        self._validate_required()")
    lines.append("        triples = list(self._type_triples())")
    lines.append("        triples.extend(self._extra_triples)")
    lines.append("        triples.extend(self._boundary_triples())")
    lines.append("        return triples")
