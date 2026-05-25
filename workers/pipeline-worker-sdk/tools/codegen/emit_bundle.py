"""Bundle-accessor emitter — read-side typed surface (FT-085 / ADR-048)."""

from __future__ import annotations

from .emit_common import (
    finalize,
    header_lines,
    is_already_emitted,
    safe_attr,
)
from .shapes import ShapeSpec


def emit_bundle_accessor(spec: ShapeSpec) -> str:
    cls = spec.target_class_local
    lines: list[str] = []
    lines.extend(header_lines(
        spec, f"Read-only bundle accessor for dec:{cls} artifacts."
    ))
    lines.append("from __future__ import annotations")
    lines.append("")
    lines.append("from dataclasses import dataclass, field")
    lines.append("")
    lines.append("")
    lines.append(f"TARGET_CLASS_IRI = {spec.target_class_iri!r}")
    lines.append("")
    lines.append("")
    lines.append("@dataclass(frozen=True)")
    lines.append(f"class {cls}Accessor:")
    lines.append(
        f'    """Read-only view of one dec:{cls} artifact in a bundle."""'
    )
    lines.append("")
    lines.append("    iri: str")
    for f in spec.fields:
        ann = (
            f"{f.python_type} | None"
            if f.single_valued
            else f"tuple[{f.python_type}, ...]"
        )
        default = " = None" if f.single_valued else " = ()"
        lines.append(f"    {safe_attr(f.local_name)}: {ann}{default}")
    for e in spec.edges:
        ann = "str | None" if e.single_valued else "tuple[str, ...]"
        default = " = None" if e.single_valued else " = ()"
        lines.append(f"    {safe_attr(e.local_name)}: {ann}{default}")
    motivational_preds = sorted({m.predicate_local for m in spec.motivational})
    for pred in motivational_preds:
        if is_already_emitted(pred, spec):
            continue
        lines.append(f"    {safe_attr(pred)}: tuple[str, ...] = ()")
    if not (spec.fields or spec.edges or motivational_preds):
        lines.append("    # No SHACL-declared body fields or edges for this type.")
        lines.append("    pass")
    lines.append("")
    lines.append("")
    lines.append("__all__ = [")
    lines.append(f'    "{cls}Accessor",')
    lines.append('    "TARGET_CLASS_IRI",')
    lines.append("]")
    return finalize(lines)
