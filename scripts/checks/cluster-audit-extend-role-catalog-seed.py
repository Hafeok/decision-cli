#!/usr/bin/env python3
"""Coherence audit for the `extend-role-catalog-seed` TaskType (FT-144).

Discriminator: a role-catalog-seed cluster touches seeds.rs / role.rs,
NOT planner.rs / inspect_dor.rs (which is extend-planner-classifier's
territory).

Audit checks:
1. `iri_constant_reachability` — every `pub const FOO_IRI: &str = ..`
   in iri_constants.rs is referenced by seed_quad_function.rs at least
   once.
2. `fail_closed_lock_in` — round_trip_tests.rs contains a test named
   `legacy_store_lookup_returns_safe_default` (locks in the ADR-069
   fail-closed guarantee per FT-121).

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


def check_iri_constant_reachability(fixture: Path) -> None:
    iri = fixture / "iri_constants.rs"
    seed = fixture / "seed_quad_function.rs"
    if not iri.exists():
        die("iri_constant_reachability", "missing iri_constants.rs")
    if not seed.exists():
        die("iri_constant_reachability", "missing seed_quad_function.rs")
    iri_body = read(iri)
    seed_body = read(seed)
    consts = set(re.findall(r"pub\s+const\s+([A-Z][A-Z_0-9]*)\s*:", iri_body))
    unreferenced = [c for c in consts if c not in seed_body]
    if unreferenced:
        die(
            "iri_constant_reachability",
            f"iri constants not referenced by seed_quad_function: {sorted(unreferenced)}",
        )


def check_fail_closed_lock_in(fixture: Path) -> None:
    tests = fixture / "round_trip_tests.rs"
    if not tests.exists():
        die("fail_closed_lock_in", "missing round_trip_tests.rs")
    body = read(tests)
    if "legacy_store_lookup_returns_safe_default" not in body:
        die(
            "fail_closed_lock_in",
            "round_trip_tests.rs is missing the legacy_store_lookup_returns_safe_default "
            "test — the load-bearing ADR-069 fail-closed guarantee lock-in",
        )


def main() -> int:
    if len(sys.argv) != 2:
        sys.stderr.write(
            "usage: cluster-audit-extend-role-catalog-seed.py <fixture_dir>\n"
        )
        return 2
    fixture = Path(sys.argv[1])
    if not fixture.is_dir():
        sys.stderr.write(f"fixture {fixture!r} is not a directory\n")
        return 2
    check_iri_constant_reachability(fixture)
    check_fail_closed_lock_in(fixture)
    print("PASS extend-role-catalog-seed (2 checks passed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
