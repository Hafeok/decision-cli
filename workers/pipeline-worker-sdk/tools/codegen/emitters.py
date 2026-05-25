"""Module-emitter facade for the SHACL→Python codegen (FT-085 / ADR-048).

The three per-shape emitters live in dedicated submodules. This file
re-exports them as a single, stable import surface for the CLI entry
point and packages the cross-cutting ``__init__.py`` emitter that wires
every per-shape class into the generated package's public surface.
"""

from __future__ import annotations

from typing import Sequence

from .emit_artifact import emit_artifact_builder
from .emit_bundle import emit_bundle_accessor
from .emit_common import (
    EDIT_BANNER,
    GENERATOR_TAG,
    finalize,
    module_basename,
    snake_case,
)
from .emit_schema import emit_pydantic_schema
from .shapes import ShapeSpec

__all__ = [
    "EDIT_BANNER",
    "GENERATOR_TAG",
    "emit_artifact_builder",
    "emit_bundle_accessor",
    "emit_package_init",
    "emit_pydantic_schema",
    "module_basename",
    "snake_case",
]


def emit_package_init(
    specs: Sequence[ShapeSpec],
    *,
    class_suffix: str,
    docstring: str,
) -> str:
    """Emit the ``__init__.py`` for one of the three generated packages."""
    lines: list[str] = []
    lines.append(f'"""{docstring}"""')
    lines.append("")
    lines.append(EDIT_BANNER.rstrip())
    lines.append(f"# Generator: {GENERATOR_TAG}")
    lines.append("")
    lines.append("from __future__ import annotations")
    lines.append("")
    for s in specs:
        mod = module_basename(s)
        cls = f"{s.target_class_local}{class_suffix}"
        lines.append(f"from .{mod} import {cls}")
    lines.append("")
    lines.append("__all__ = [")
    for s in specs:
        cls = f"{s.target_class_local}{class_suffix}"
        lines.append(f'    "{cls}",')
    lines.append("]")
    return finalize(lines)
