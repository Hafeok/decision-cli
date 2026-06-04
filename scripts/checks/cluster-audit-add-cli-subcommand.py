#!/usr/bin/env python3
"""Coherence audit for the `add-cli-subcommand` TaskType (FT-142).

Discriminator: a CLI-subcommand cluster MUST emit an integration test
under `crates/decision-cli/tests/`. Catches misclassification with
artifact-type clusters (which use unit tests under `src/`) and worker
task types (which use Python tests).

Audit checks:
1. `integration_test_path` — the cluster emits at least one file at
   `crates/decision-cli/tests/*.rs`. (Discriminator.)
2. `flags_tested` — every `pub` field on `clap_args_module`'s struct
   appears at least once in the integration test (as long-flag or
   field-name reference).

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


def check_integration_test_path(fixture: Path) -> Path:
    """Discriminator vs artifact-type / worker clusters."""
    candidates = list((fixture / "crates" / "decision-cli" / "tests").glob("*.rs"))
    if not candidates:
        die(
            "integration_test_path",
            "no integration test under crates/decision-cli/tests/ — "
            "did you mean add-artifact-type (unit tests under src/)?",
        )
    return candidates[0]


def check_flags_tested(fixture: Path, integration_test: Path) -> None:
    args_file = next(
        (
            p
            for p in fixture.rglob("*.rs")
            if "args" in p.stem.lower() or "clap" in p.stem.lower()
        ),
        None,
    )
    if args_file is None:
        die("flags_tested", "no clap args module in fixture (*.rs with 'args' or 'clap')")
    args_body = read(args_file)
    test_body = read(integration_test)
    fields = set(
        re.findall(r"^\s*pub\s+([A-Za-z_][A-Za-z0-9_]*)\s*:", args_body, re.MULTILINE)
    )
    untested = []
    for f in fields:
        long_flag = f"--{f.replace('_', '-')}"
        if f not in test_body and long_flag not in test_body:
            untested.append(f)
    if untested:
        die(
            "flags_tested",
            f"integration test does not reference flags: {sorted(untested)}",
        )


def main() -> int:
    if len(sys.argv) != 2:
        sys.stderr.write("usage: cluster-audit-add-cli-subcommand.py <fixture_dir>\n")
        return 2
    fixture = Path(sys.argv[1])
    if not fixture.is_dir():
        sys.stderr.write(f"fixture {fixture!r} is not a directory\n")
        return 2
    test_file = check_integration_test_path(fixture)
    check_flags_tested(fixture, test_file)
    print("PASS add-cli-subcommand (2 checks passed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
