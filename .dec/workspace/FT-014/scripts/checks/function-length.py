#!/usr/bin/env python3
"""Python function length enforcement for ADR-013 Rule 2.

Exits 0 if all functions under workers/*/ are within limits.
Exits 1 if any function exceeds FN_LENGTH_HARD (default 40 statement nodes).
Functions in the FN_LENGTH_WARN–FN_LENGTH_HARD band (default 30–40) produce
advisory WARNING diagnostics on stdout but do not change exit code.
"""

import ast
import os
import sys
from pathlib import Path

# Configurable thresholds
FN_LENGTH_HARD = int(os.environ.get("FN_LENGTH_HARD", "40"))
FN_LENGTH_WARN = int(os.environ.get("FN_LENGTH_WARN", "30"))


def count_statements(node):
    """Count statement nodes in a function body."""
    count = 0
    for child in ast.walk(node):
        if isinstance(child, ast.stmt):
            count += 1
    # Subtract 1 for the function def itself
    return max(0, count - 1)


def check_file(file_path):
    """Check function lengths in a single Python file.

    Returns (errors, warnings) tuple of counts.
    """
    errors = 0
    warnings = 0

    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            source = f.read()

        tree = ast.parse(source, filename=str(file_path))

        for node in ast.walk(tree):
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                stmt_count = count_statements(node)

                if stmt_count > FN_LENGTH_HARD:
                    print(
                        f"ERROR: {file_path}:{node.lineno}: function {node.name} "
                        f"has {stmt_count} statement nodes (limit {FN_LENGTH_HARD})"
                    )
                    errors += 1
                elif stmt_count > FN_LENGTH_WARN:
                    print(
                        f"WARNING: {file_path}:{node.lineno}: function {node.name} "
                        f"has {stmt_count} statement nodes (warn threshold {FN_LENGTH_WARN})"
                    )
                    warnings += 1

    except SyntaxError as e:
        print(f"ERROR: {file_path}: syntax error at line {e.lineno}: {e.msg}", file=sys.stderr)
        errors += 1
    except Exception as e:
        print(f"ERROR: {file_path}: {e}", file=sys.stderr)
        errors += 1

    return errors, warnings


def main():
    """Walk workers/*/ and check all Python files."""
    workers_dir = Path("workers")

    if not workers_dir.exists():
        print("OK: no workers/ directory found")
        return 0

    total_errors = 0
    total_warnings = 0

    # Find all first-party Python files
    # Exclude: tests/, __pycache__/, .venv/
    for py_file in workers_dir.rglob("*.py"):
        # Skip excluded directories
        if any(part in py_file.parts for part in ["tests", "__pycache__", ".venv", ".pytest_cache"]):
            continue

        errors, warnings = check_file(py_file)
        total_errors += errors
        total_warnings += warnings

    if total_errors > 0:
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
