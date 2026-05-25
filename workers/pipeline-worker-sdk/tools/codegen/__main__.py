"""CLI entry point for the SHACL→Python codegen (FT-085 / ADR-048).

Invoked as either ``python -m tools.codegen`` from the SDK package root,
or via the ``codegen`` script declared in ``pyproject.toml``.

Reads SHACL shape files from the configured directory (default:
``workers/_shared/shapes/`` relative to repo root, override via
``--shapes-dir`` or ``DEC_SHAPES_DIR``) and writes three module trees
under ``src/pipeline_worker_sdk/`` :

* ``bundle/_generated/`` — read-only accessors
* ``artifact/_generated/`` — typed builders
* ``schemas/_generated/`` — Pydantic models

The output is byte-stable: running twice produces no diff. CI on the
SDK repo runs ``codegen --check`` to enforce that the checked-in
generated files match what the shapes currently produce.
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path
from typing import Iterable

from .emitters import (
    emit_artifact_builder,
    emit_bundle_accessor,
    emit_package_init,
    emit_pydantic_schema,
    module_basename,
)
from .shapes import ShapeSpec, load_shapes


# Paths the generator targets, relative to the SDK package root.
SDK_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SHAPES_DIR = SDK_ROOT.parent / "_shared" / "shapes"
BUNDLE_OUT = SDK_ROOT / "src" / "pipeline_worker_sdk" / "bundle" / "_generated"
ARTIFACT_OUT = SDK_ROOT / "src" / "pipeline_worker_sdk" / "artifact" / "_generated"
SCHEMAS_OUT = SDK_ROOT / "src" / "pipeline_worker_sdk" / "schemas" / "_generated"

# Files inside `_generated` we manage. Anything else gets cleaned up to
# keep the output set deterministic.
GENERATED_DIRS = (BUNDLE_OUT, ARTIFACT_OUT, SCHEMAS_OUT)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="codegen")
    parser.add_argument(
        "--shapes-dir",
        type=Path,
        default=Path(os.environ.get("DEC_SHAPES_DIR") or DEFAULT_SHAPES_DIR),
        help="Directory containing SHACL shape TTL files.",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help=(
            "Do not write files. Exit 0 if the on-disk generated tree matches "
            "what would be produced, exit 1 otherwise."
        ),
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress per-file informational output.",
    )
    args = parser.parse_args(argv)

    shapes_dir: Path = args.shapes_dir
    if not shapes_dir.is_dir():
        print(f"error: shapes-dir not found: {shapes_dir}", file=sys.stderr)
        return 2

    specs = load_shapes(shapes_dir)
    if not specs:
        print(f"error: no per-type shape files found in {shapes_dir}", file=sys.stderr)
        return 2

    expected = _build_expected_tree(specs)

    if args.check:
        ok = _check_tree(expected)
        if ok:
            if not args.quiet:
                print("codegen --check: generated tree is up to date.")
            return 0
        return 1

    _ensure_dirs()
    _write_tree(expected, quiet=args.quiet)
    if not args.quiet:
        print(f"codegen: wrote {len(expected)} files across 3 packages.")
    return 0


# ---------------------------------------------------------------------------
# Tree building / writing / checking
# ---------------------------------------------------------------------------


def _build_expected_tree(specs: Iterable[ShapeSpec]) -> dict[Path, str]:
    """Compute the final {path: content} map without touching the filesystem."""
    specs = list(specs)
    tree: dict[Path, str] = {}
    for spec in specs:
        mod = module_basename(spec)
        tree[BUNDLE_OUT / f"{mod}.py"] = emit_bundle_accessor(spec)
        tree[ARTIFACT_OUT / f"{mod}.py"] = emit_artifact_builder(spec)
        tree[SCHEMAS_OUT / f"{mod}.py"] = emit_pydantic_schema(spec)
    # Package __init__ files
    tree[BUNDLE_OUT / "__init__.py"] = emit_package_init(
        specs,
        class_suffix="Accessor",
        docstring="Generated read-only bundle accessors keyed by artifact type.",
    )
    tree[ARTIFACT_OUT / "__init__.py"] = emit_package_init(
        specs,
        class_suffix="Builder",
        docstring="Generated typed artifact builders keyed by artifact type.",
    )
    tree[SCHEMAS_OUT / "__init__.py"] = emit_package_init(
        specs,
        class_suffix="Schema",
        docstring=(
            "Generated Pydantic models for structured-output coupling with the "
            "SDK's typed surfaces."
        ),
    )
    return tree


def _ensure_dirs() -> None:
    for d in GENERATED_DIRS:
        d.mkdir(parents=True, exist_ok=True)


def _write_tree(tree: dict[Path, str], *, quiet: bool) -> None:
    # Stable iteration order so logs are reproducible across runs.
    for path in sorted(tree, key=lambda p: p.as_posix()):
        path.parent.mkdir(parents=True, exist_ok=True)
        existing = path.read_text(encoding="utf-8") if path.exists() else None
        if existing == tree[path]:
            if not quiet:
                print(f"  unchanged {path.relative_to(SDK_ROOT)}")
            continue
        path.write_text(tree[path], encoding="utf-8")
        if not quiet:
            print(f"  wrote     {path.relative_to(SDK_ROOT)}")
    # Remove stray files in the managed dirs that aren't in the expected set.
    expected_set = {p.resolve() for p in tree}
    for d in GENERATED_DIRS:
        if not d.exists():
            continue
        for found in d.iterdir():
            if not found.is_file():
                continue
            if found.resolve() not in expected_set:
                found.unlink()
                if not quiet:
                    print(f"  removed   {found.relative_to(SDK_ROOT)}")


def _check_tree(expected: dict[Path, str]) -> bool:
    """Return True iff every expected file exists and matches byte-for-byte."""
    ok = True
    expected_set = {p.resolve() for p in expected}
    for path, content in sorted(expected.items(), key=lambda kv: kv[0].as_posix()):
        if not path.exists():
            print(f"missing: {path.relative_to(SDK_ROOT)}", file=sys.stderr)
            ok = False
            continue
        actual = path.read_text(encoding="utf-8")
        if actual != content:
            print(f"drift:   {path.relative_to(SDK_ROOT)}", file=sys.stderr)
            ok = False
    # Stray files
    for d in GENERATED_DIRS:
        if not d.exists():
            continue
        for found in d.iterdir():
            if found.is_file() and found.resolve() not in expected_set:
                print(f"stray:   {found.relative_to(SDK_ROOT)}", file=sys.stderr)
                ok = False
    return ok


if __name__ == "__main__":  # pragma: no cover - CLI dispatch
    sys.exit(main())
