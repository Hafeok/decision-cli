#!/usr/bin/env python3
"""Coherence audit for the `add-artifact-type` TaskType (FT-141).

Discriminator: an artifact-type cluster touches Rust + Turtle only —
NO .py files. Catches misclassification with worker task types
(add-judge-worker, add-author-worker) which all emit Python.

FT-172 hardening (the two blind spots witnessed on FT-147):

- canonical_namespace — every IRI the cells emit must use a known base
  (decision-cli.dev, W3C, PROV, Dublin Core). Catches worker-invented
  vocabularies before the operator does.
- compile_probe — the emitted Rust must type-check when grafted onto
  HEAD in a temporary git worktree, with module declarations auto-wired
  along each new file's ancestor chain. Catches non-compiling emissions
  the structural checks cannot see.

Usage: cluster-audit-add-artifact-type.py <fixture_dir> [cell_path ...]
The optional cell paths (relative to the fixture) are the harness-
resolved output_paths (FT-166/FT-170); content checks audit exactly
that set and ignore worker-fabricated extras. Without them, content
checks fall back to every .rs/.ttl in the fixture.

Exit 0/1/2 per ADR-013 contract.
"""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
import tempfile
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
    rs_files = list(fixture.rglob("*.rs"))
    if not rs_files:
        die(
            "rust_struct",
            "no .rs files in fixture; artifact-type cluster emits Rust struct + parser + emitter",
        )
    ttl_files = list(fixture.rglob("*.ttl"))
    if not ttl_files:
        die("shacl_shape", "no .ttl SHACL shape in fixture")


def check_shacl_covers_struct_fields(fixture: Path) -> None:
    struct_file = next((f for f in fixture.rglob("*.rs") if "struct" in f.stem), None)
    if struct_file is None:
        return  # No struct to compare; struct-presence check already fired.
    struct_body = read(struct_file)
    shape_body = read(next(fixture.rglob("*.ttl")))
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


# FT-172: IRI bases the dec vocabulary legitimately touches. Anything
# else in an emitted .rs/.ttl is a worker-invented namespace (witnessed
# on FT-147: `decisionframework.com`).
ALLOWED_IRI_BASES = (
    "https://decision-cli.dev/",
    "http://www.w3.org/",
    "http://purl.org/dc/",
    "https://github.com/",  # provenance links in doc comments
    "urn:dec:",
)

IRI_RE = re.compile(r"""(?:https?:)//[^\s"'<>()\\]+|urn:dec:[^\s"'<>()\\]+""")


def audited_files(fixture: Path, cell_paths: list[Path], suffixes: tuple[str, ...]) -> list[Path]:
    """The declared cell set when provided (FT-170 guarantees presence),
    else every matching file in the fixture."""
    if cell_paths:
        return [fixture / p for p in cell_paths if p.suffix in suffixes and (fixture / p).exists()]
    return [p for s in suffixes for p in fixture.rglob(f"*{s}")]


def check_canonical_namespace(fixture: Path, cell_paths: list[Path]) -> None:
    offenders = []
    for f in audited_files(fixture, cell_paths, (".rs", ".ttl")):
        for lineno, line in enumerate(read(f).splitlines(), start=1):
            for iri in IRI_RE.findall(line):
                if not iri.startswith(ALLOWED_IRI_BASES):
                    offenders.append(f"{f.relative_to(fixture)}:{lineno}: {iri}")
    if offenders:
        die(
            "canonical_namespace",
            "non-canonical IRI base(s) in emitted files (expected "
            "https://decision-cli.dev/ns…): " + "; ".join(offenders[:10]),
        )


def declaring_file(worktree: Path, directory: Path) -> Path:
    """The file that declares `directory` as a module: its mod.rs, or a
    sibling `<dir>.rs` (2018-style), creating mod.rs when neither exists."""
    mod_rs = directory / "mod.rs"
    sibling = directory.parent / f"{directory.name}.rs"
    if mod_rs.exists():
        return mod_rs
    if sibling.exists():
        return sibling
    mod_rs.parent.mkdir(parents=True, exist_ok=True)
    mod_rs.write_text("//! Module auto-wired by the FT-172 compile probe.\n", encoding="utf-8")
    # The new mod.rs itself needs declaring one level up.
    wire_module_path(worktree, mod_rs)
    return mod_rs


def wire_module_path(worktree: Path, rs_file: Path) -> None:
    """Ensure `rs_file` is reachable from its crate root: append
    `pub mod <name>;` to each ancestor's declaring file when absent."""
    rel = rs_file.relative_to(worktree)
    parts = rel.parts  # crates/<crate>/src/<...>/<name>.rs
    if len(parts) < 4 or parts[0] != "crates" or parts[2] != "src":
        return
    name = rs_file.stem
    if name in ("mod", "lib", "main"):
        return
    decl_target = declaring_file(worktree, rs_file.parent) if parts[3:] != (f"{name}.rs",) else None
    if rs_file.parent == worktree / parts[0] / parts[1] / "src":
        decl_target = worktree / parts[0] / parts[1] / "src" / "lib.rs"
    if decl_target is None or decl_target == rs_file:
        return
    body = decl_target.read_text(encoding="utf-8")
    if re.search(rf"\bmod\s+{re.escape(name)}\s*;", body):
        return
    decl = f"#[cfg(test)]\nmod {name};\n" if name == "tests" else f"pub mod {name};\n"
    decl_target.write_text(body + "\n" + decl, encoding="utf-8")


def check_compile_probe(fixture: Path, cell_paths: list[Path]) -> None:
    """Graft the emitted Rust onto HEAD in a temp worktree and require
    `cargo check -p dec-ontology --all-targets` to pass."""
    repo_root = Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True, text=True, check=False,
        ).stdout.strip()
        or "."
    )
    overlay = [
        p for p in (cell_paths or [q.relative_to(fixture) for q in fixture.rglob("*")])
        if str(p).startswith("crates/") and (fixture / p).is_file()
    ]
    if not any(str(p).endswith(".rs") for p in overlay):
        return  # nothing graftable; struct-presence check already governs
    worktree = Path(tempfile.mkdtemp(prefix="dec-audit-probe-"))
    try:
        add = subprocess.run(
            ["git", "worktree", "add", "--detach", str(worktree), "HEAD"],
            cwd=repo_root, capture_output=True, text=True, check=False,
        )
        if add.returncode != 0:
            die("compile_probe", f"worktree setup failed: {add.stderr.strip()}", code=2)
        for rel in overlay:
            target = worktree / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(fixture / rel, target)
        for rel in overlay:
            if str(rel).endswith(".rs"):
                wire_module_path(worktree, worktree / rel)
        probe = subprocess.run(
            ["cargo", "check", "-p", "dec-ontology", "--all-targets", "--quiet"],
            cwd=worktree, capture_output=True, text=True, check=False, timeout=240,
            env={**__import__("os").environ, "CARGO_TARGET_DIR": str(repo_root / "target")},
        )
        if probe.returncode != 0:
            tail = "\n".join(probe.stderr.splitlines()[-40:])
            die("compile_probe", f"emitted Rust does not compile against HEAD:\n{tail}")
    except subprocess.TimeoutExpired:
        die("compile_probe", "cargo check timed out", code=2)
    except OSError as e:
        die("compile_probe", f"probe environment failure: {e}", code=2)
    finally:
        subprocess.run(
            ["git", "worktree", "remove", "--force", str(worktree)],
            cwd=repo_root, capture_output=True, check=False,
        )
        shutil.rmtree(worktree, ignore_errors=True)


def main() -> int:
    if len(sys.argv) < 2:
        sys.stderr.write(
            "usage: cluster-audit-add-artifact-type.py <fixture_dir> [cell_path ...]\n"
        )
        return 2
    fixture = Path(sys.argv[1])
    if not fixture.is_dir():
        sys.stderr.write(f"fixture {fixture!r} is not a directory\n")
        return 2
    cell_paths = [Path(a) for a in sys.argv[2:]]
    check_no_python_files(fixture)
    check_struct_and_shape_present(fixture)
    check_shacl_covers_struct_fields(fixture)
    check_canonical_namespace(fixture, cell_paths)
    check_compile_probe(fixture, cell_paths)
    print("PASS add-artifact-type (5 checks passed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
