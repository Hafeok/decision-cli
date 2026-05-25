"""Shared helpers across the three emitter modules (FT-085 / ADR-048)."""

from __future__ import annotations

import re
from typing import Iterable

from .shapes import ShapeSpec

GENERATOR_TAG = "pipeline-worker-sdk codegen (FT-085 / ADR-048)"
EDIT_BANNER = (
    "# ============================================================\n"
    "# GENERATED FILE — DO NOT EDIT BY HAND.\n"
    "# Regenerate via:  uv run codegen\n"
    "# ============================================================\n"
)


def snake_case(camel: str) -> str:
    s = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", camel)
    s = re.sub(r"([a-z\d])([A-Z])", r"\1_\2", s)
    return s.lower()


def module_basename(spec: ShapeSpec) -> str:
    return snake_case(spec.target_class_local)


_PYTHON_KEYWORDS = {
    "class", "from", "import", "return", "yield", "as", "is", "in", "and", "or",
    "not", "for", "while", "if", "else", "elif", "try", "except", "finally",
    "with", "lambda", "pass", "raise", "global", "nonlocal", "def",
}


def safe_attr(name: str) -> str:
    """Make a valid Python attribute name out of a predicate local name."""
    out = re.sub(r"[^A-Za-z0-9_]", "_", name)
    if out in _PYTHON_KEYWORDS or (out and out[0].isdigit()):
        out = f"f_{out}"
    return out


def const_safe(name: str) -> str:
    """Pythonic constant suffix from a predicate local name."""
    return re.sub(r"[^A-Za-z0-9_]", "_", name)


def doc(local: str) -> str:
    return f"dec:{local}"


def is_already_emitted(pred_local: str, spec: ShapeSpec) -> bool:
    """A motivational predicate already covered as a body field or edge."""
    if any(f.local_name == pred_local for f in spec.fields):
        return True
    if any(e.local_name == pred_local for e in spec.edges):
        return True
    return False


def finalize(lines: Iterable[str]) -> str:
    """Strip trailing whitespace per line, ensure a single trailing newline."""
    cleaned = [line.rstrip() for line in lines]
    text = "\n".join(cleaned).rstrip() + "\n"
    return text


def header_lines(spec: ShapeSpec, docstring: str) -> list[str]:
    """Shared file header: docstring + banner + source-shape comment."""
    return [
        f'"""{docstring}"""',
        "",
        EDIT_BANNER.rstrip(),
        f"# Source SHACL shape: workers/_shared/shapes/{spec.source_file}",
        f"# Generator: {GENERATOR_TAG}",
        "",
    ]
