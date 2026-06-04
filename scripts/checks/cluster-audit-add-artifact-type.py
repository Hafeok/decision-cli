#!/usr/bin/env python3
"""Coherence audit for the `add-artifact-type` TaskType (FT-141).

Discriminator: an artifact-type cluster touches Rust + Turtle only —
NO .py files. Catches misclassification with worker task types
(add-judge-worker, add-author-worker) which all emit Python.

Exit 0/1/2 per ADR-013 contract.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


def die(check: str, detail: str, code: int = 1) -> None:
    sys.stderr.write(f"FAIL check={check}: {detail}\n")
    sys.exit(code)


def read(p: Path) -> str:
    try:
        return p.read_text(encoding="utf-8")
    except OSError as e:
        die("missing_file", f"{p}: {e}", code=2)
        return ""


def check_no_python_files(fixture: Path) -> None:
    """Discriminator vs worker task types."""
    py_files = list(fixture.rglob("*.py"))
    if py_files:
        names = ", ".join(p.name for p in py_files)
        die(
            "no_python_files",
            f"artifact-type cluster must not emit Python files; found {names} — "
            "did you mean add-judge-worker / add-author-worker?",
        )


def check_struct_and_shape_present(fixture: Path) -> None:
    rs_files = list(fixture.glob("*.rs"))
    if not rs_files:
        die(
            "rust_struct",
            "no .rs files in fixture; artifact-type cluster emits Rust struct + parser + emitter",
        )
    ttl_files = list(fixture.glob("*.ttl"))
    if not ttl_files:
        die("shacl_shape", "no .ttl SHACL shape in fixture")


def check_shacl_covers_struct_fields(fixture: Path) -> None:
    struct_file = next((f for f in fixture.glob("*.rs") if "struct" in f.stem), None)
    if struct_file is None:
        return  # No struct to compare; struct-presence check already fired.
    struct_body = read(struct_file)
    shape_body = read(next(fixture.glob("*.ttl")))
    fields = set(
        re.findall(r"^\s*pub\s+([A-Za-z_][A-Za-z0-9_]*)\s*:", struct_body, re.MULTILINE)
    )
    paths = set(re.findall(r"sh:path\s+dec:([A-Za-z_][A-Za-z0-9_]*)", shape_body))
    # camelCase to snake_case for property paths.
    paths_snake = {
        re.sub(r"(?<!^)(?=[A-Z])", "_", p).lower() for p in paths
    } | paths
    missing = fields - paths_snake
    if missing:
        die(
            "shacl_field_coverage",
            f"SHACL shape lacks sh:path for struct field(s): {sorted(missing)}",
        )


def main() -> int:
    if len(sys.argv) != 2:
        sys.stderr.write("usage: cluster-audit-add-artifact-type.py <fixture_dir>\n")
        return 2
    fixture = Path(sys.argv[1])
    if not fixture.is_dir():
        sys.stderr.write(f"fixture {fixture!r} is not a directory\n")
        return 2
    check_no_python_files(fixture)
    check_struct_and_shape_present(fixture)
    check_shacl_covers_struct_fields(fixture)
    print("PASS add-artifact-type (3 checks passed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
