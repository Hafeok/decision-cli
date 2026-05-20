#!/usr/bin/env python3
"""Enforce ADR-013 Rule 2 — Function Length Limit for Python sources.

Walks every ``*.py`` file under ``workers/*/`` (excluding ``tests/``,
``__pycache__/``, and ``.venv/``) and counts statement nodes inside each
``FunctionDef`` / ``AsyncFunctionDef`` body. The script propagates the
worst severity it sees as its exit code (1=hard, 2=warn, 0=clean).

Thresholds may be overridden via environment variables ``FN_LENGTH_HARD``
(default 40) and ``FN_LENGTH_WARN`` (default 30) — both match the Rust
companion (``function-length.sh``) so the contract is uniform across
implementation surfaces per ADR-013.

Diagnostic messages are written to ``stdout`` so ``product verify``
captures them in the TC failure record. Script-self errors go to
``stderr``.
"""

from __future__ import annotations

import ast
import os
import subprocess
import sys
from pathlib import Path


HARD_LIMIT = int(os.environ.get("FN_LENGTH_HARD", "40"))
WARN_LIMIT = int(os.environ.get("FN_LENGTH_WARN", "30"))


def repo_root() -> Path:
    """Resolve the repository root via git, falling back to CWD."""
    try:
        out = subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"],
            stderr=subprocess.DEVNULL,
        )
        return Path(out.decode().strip())
    except (subprocess.CalledProcessError, FileNotFoundError):
        return Path.cwd()


def collect_files(root: Path) -> list[Path]:
    """Return first-party Python sources excluding tests and venv junk."""
    workers = root / "workers"
    if not workers.is_dir():
        return []
    out: list[Path] = []
    for p in workers.rglob("*.py"):
        parts = set(p.parts)
        if "tests" in parts or "__pycache__" in parts or ".venv" in parts:
            continue
        out.append(p)
    return sorted(out)


def count_statements(node: ast.AST) -> int:
    """Count statement nodes in a function body (recursive, depth-blind)."""
    count = 0
    for child in ast.walk(node):
        if isinstance(child, ast.stmt) and child is not node:
            count += 1
    return count


def find_functions(tree: ast.AST) -> list[tuple[str, int, int]]:
    """Yield (qualified_name, lineno, statement_count) for each function."""
    out: list[tuple[str, int, int]] = []
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            out.append((node.name, node.lineno, count_statements(node)))
    return out


def main() -> int:
    root = repo_root()
    os.chdir(root)
    files = collect_files(root)
    if not files:
        print("OK: no first-party Python source files found")
        return 0

    errors: list[tuple[Path, int, str, int]] = []
    warns: list[tuple[Path, int, str, int]] = []

    for path in files:
        try:
            source = path.read_text(encoding="utf-8")
        except OSError as exc:
            print(f"ERROR: cannot read {path}: {exc}", file=sys.stderr)
            return 127
        try:
            tree = ast.parse(source, filename=str(path))
        except SyntaxError as exc:
            print(f"ERROR: cannot parse {path}: {exc}", file=sys.stderr)
            return 127
        for name, lineno, stmts in find_functions(tree):
            if stmts > HARD_LIMIT:
                errors.append((path, lineno, name, stmts))
            elif stmts > WARN_LIMIT:
                warns.append((path, lineno, name, stmts))

    if errors:
        print(f"ERROR: functions exceeding hard limit ({HARD_LIMIT} statement lines):")
        for path, lineno, name, stmts in sorted(errors, key=lambda r: -r[3]):
            rel = path.relative_to(root)
            print(f"  {rel}:{lineno} {name} — {stmts} statement lines (limit: {HARD_LIMIT})")
        return 1

    if warns:
        print(f"WARNING: functions approaching limit ({WARN_LIMIT}-{HARD_LIMIT} statement lines):")
        for path, lineno, name, stmts in sorted(warns, key=lambda r: -r[3]):
            rel = path.relative_to(root)
            print(f"  {rel}:{lineno} {name} — {stmts} statement lines (warn at: {WARN_LIMIT})")
        return 2

    print(f"OK: all Python functions within limits (hard={HARD_LIMIT}, warn={WARN_LIMIT})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
