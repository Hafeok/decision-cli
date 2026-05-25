"""Pydantic-schema emitter — structured-output coupling (FT-085 / ADR-048)."""

from __future__ import annotations

from .emit_common import (
    doc,
    finalize,
    header_lines,
    is_already_emitted,
    safe_attr,
)
from .shapes import ShapeSpec


def emit_pydantic_schema(spec: ShapeSpec) -> str:
    cls = spec.target_class_local
    motivational_preds = sorted({m.predicate_local for m in spec.motivational})
    lines: list[str] = []
    lines.extend(header_lines(
        spec, f"Pydantic model for dec:{cls} (structured-output schema)."
    ))
    lines.append("from __future__ import annotations")
    lines.append("")
    lines.append("from pydantic import BaseModel, Field")
    lines.append("")
    lines.append("")
    lines.append(f"TARGET_CLASS_IRI = {spec.target_class_iri!r}")
    lines.append("")
    lines.append("")
    lines.append(f"class {cls}Schema(BaseModel):")
    lines.append(f'    """Pydantic schema for one dec:{cls} artifact."""')
    lines.append("")
    lines.append('    iri: str = Field(..., description="The artifact IRI.")')
    _emit_field_lines(lines, spec)
    _emit_edge_lines(lines, spec)
    _emit_motivational_lines(lines, spec, motivational_preds)
    if not (spec.fields or spec.edges or motivational_preds):
        lines.append("    # No SHACL-declared body fields, edges, or motivational")
        lines.append("    # alternatives for this type.")
        lines.append("    pass")
    lines.append("")
    lines.append("    model_config = {")
    lines.append('        "extra": "forbid",')
    lines.append("    }")
    lines.append("")
    lines.append("")
    lines.append("__all__ = [")
    lines.append(f'    "{cls}Schema",')
    lines.append('    "TARGET_CLASS_IRI",')
    lines.append("]")
    return finalize(lines)


def _emit_field_lines(lines: list[str], spec: ShapeSpec) -> None:
    for f in spec.fields:
        attr = safe_attr(f.local_name)
        if f.single_valued and f.required:
            lines.append(
                f"    {attr}: {f.python_type} = Field("
                f"..., description={doc(f.local_name)!r})"
            )
        elif f.single_valued:
            lines.append(
                f"    {attr}: {f.python_type} | None = Field("
                f"default=None, description={doc(f.local_name)!r})"
            )
        else:
            lines.append(
                f"    {attr}: list[{f.python_type}] = Field("
                f"default_factory=list, description={doc(f.local_name)!r})"
            )


def _emit_edge_lines(lines: list[str], spec: ShapeSpec) -> None:
    for e in spec.edges:
        attr = safe_attr(e.local_name)
        if e.single_valued:
            lines.append(
                f"    {attr}: str | None = Field("
                f"default=None, description={doc(e.local_name)!r})"
            )
        else:
            lines.append(
                f"    {attr}: list[str] = Field("
                f"default_factory=list, description={doc(e.local_name)!r})"
            )


def _emit_motivational_lines(
    lines: list[str], spec: ShapeSpec, motivational_preds: list[str]
) -> None:
    for pred in motivational_preds:
        if is_already_emitted(pred, spec):
            continue
        lines.append(
            f"    {safe_attr(pred)}: list[str] = Field("
            f"default_factory=list, "
            f'description="motivational edge: {pred}")'
        )
