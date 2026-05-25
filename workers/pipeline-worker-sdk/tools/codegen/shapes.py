"""Shape-loader facade for the codegen pipeline (FT-085 / ADR-048).

Iterates the per-type SHACL TTL files in a configured directory, parses
each into a :class:`ShapeSpec`, and returns the catalog in deterministic
order. Parsing internals live in :mod:`shapes_parser`; type definitions
and SHACL/RDF constants live in :mod:`shapes_types`.
"""

from __future__ import annotations

from pathlib import Path
from typing import List, Sequence

from .shapes_parser import extract_specs_for_file
from .shapes_types import (
    BOUNDARY_ARTIFACT,
    DATATYPE_TO_PYTHON,
    NS_DEC,
    NS_RDF,
    NS_SH,
    NS_XSD,
    EdgeField,
    MotivationalAlternative,
    PropertyField,
    ShapeSpec,
)

__all__ = [
    "BOUNDARY_ARTIFACT",
    "DATATYPE_TO_PYTHON",
    "EdgeField",
    "MotivationalAlternative",
    "NS_DEC",
    "NS_RDF",
    "NS_SH",
    "NS_XSD",
    "PropertyField",
    "SKIP_FILES",
    "ShapeSpec",
    "format_specs",
    "load_shapes",
]

# Universal/boundary/motivational fragments are loaded by the harness's
# ontology bootstrap, not by the per-type emitter pipeline. Skip them.
SKIP_FILES = {
    "mechanical-provenance.ttl",
    "motivational-predicates.ttl",
    "boundary-artifact.ttl",
    "manifest.ttl",
}


def load_shapes(shapes_dir: Path) -> List[ShapeSpec]:
    """Parse every per-type TTL under ``shapes_dir`` and return ordered specs."""
    specs: List[ShapeSpec] = []
    for path in sorted(shapes_dir.glob("*.ttl")):
        if path.name in SKIP_FILES:
            continue
        specs.extend(extract_specs_for_file(path))
    specs.sort(key=lambda s: s.target_class_local)
    return specs


def format_specs(specs: Sequence[ShapeSpec]) -> str:
    """Pretty-print a catalog for debugging."""
    lines: list[str] = []
    for s in specs:
        lines.append(f"=== {s.target_class_local} <{s.target_class_iri}> ===")
        for f in s.fields:
            lines.append(
                f"  field {f.local_name}: {f.python_type} "
                f"required={f.required} single={f.single_valued}"
            )
        for e in s.edges:
            lines.append(
                f"  edge  {e.local_name} -> {e.range_class_local or '*'} "
                f"required={e.required} single={e.single_valued}"
            )
        for m in s.motivational:
            lines.append(
                f"  motivational {m.predicate_local} -> {m.target_class_local}"
            )
        lines.append(f"  accepts_boundary={s.accepts_boundary}")
    return "\n".join(lines)
