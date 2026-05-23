#!/usr/bin/env python3
"""Seed `dec:Capability` + `dec:RoleBinding` catalog from `config/*.yaml`.

FT-058 / ADR-036 — the operator-facing entry point for catalog bootstrap.
Shells out to the `dec _bootstrap-catalog` hidden subcommand, which holds
the SHACL chokepoint and the atomic-transaction guarantee in Rust.

Per ADR-036 the YAML at `config/capabilities.yaml` and
`config/role-bindings.yaml` is documentation and bootstrap input; after
the first run the graph is authoritative. Re-runs with unchanged YAML
are idempotent no-ops. Re-runs whose YAML diverges from the graph error
out without writing — bootstrap never silently overwrites graph state.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_CAPABILITIES_YAML = REPO_ROOT / "config" / "capabilities.yaml"
DEFAULT_BINDINGS_YAML = REPO_ROOT / "config" / "role-bindings.yaml"


def main() -> int:
    args = parse_args()
    workdir = resolve_workdir(args.graph_path)
    dec_bin = resolve_dec_binary(args.dec_binary)
    if not (workdir / ".dec" / "store" / "orchestration.nq").exists():
        sys.stderr.write(
            "bootstrap_catalog: no orchestration store at "
            f"{workdir}/.dec/store/orchestration.nq\n"
            "  Initialise the store first with `dec init`.\n"
        )
        return 1
    return invoke_dec(dec_bin, workdir, args)


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        prog="bootstrap_catalog.py",
        description=(
            "Bootstrap the dec:Capability + dec:RoleBinding catalog into "
            "an already-initialised orchestration store (FT-058)."
        ),
    )
    p.add_argument(
        "--graph-path",
        type=Path,
        required=True,
        help=(
            "Path to the working directory containing .dec/store/. The "
            "orchestration store must already be initialised."
        ),
    )
    p.add_argument(
        "--capabilities",
        type=Path,
        default=DEFAULT_CAPABILITIES_YAML,
        help=f"Path to capabilities YAML (default: {DEFAULT_CAPABILITIES_YAML}).",
    )
    p.add_argument(
        "--bindings",
        type=Path,
        default=DEFAULT_BINDINGS_YAML,
        help=f"Path to role-bindings YAML (default: {DEFAULT_BINDINGS_YAML}).",
    )
    p.add_argument(
        "--migrate",
        action="store_true",
        help=(
            "Also backfill `dec:stakes = routine` on existing bundles and "
            "the FT-057 token-breakdown fields on existing sessions."
        ),
    )
    p.add_argument(
        "--dry-run",
        action="store_true",
        help="Validate and plan but do not write the updated store back to disk.",
    )
    p.add_argument(
        "--dec-binary",
        type=Path,
        default=None,
        help=(
            "Override the `dec` binary path (defaults to $DEC_BINARY, "
            "then `dec` on PATH, then ./target/debug/dec)."
        ),
    )
    return p.parse_args()


def resolve_workdir(graph_path: Path) -> Path:
    """Accept either the workdir (containing `.dec/`) or the store
    directory itself; resolve to the workdir form `dec` expects."""
    resolved = graph_path.resolve()
    if resolved.name == "store" and resolved.parent.name == ".dec":
        return resolved.parent.parent
    if resolved.name == ".dec":
        return resolved.parent
    return resolved


def resolve_dec_binary(override: Path | None) -> Path:
    if override is not None:
        return override.resolve()
    env_val = os.environ.get("DEC_BINARY")
    if env_val:
        return Path(env_val).resolve()
    on_path = shutil.which("dec")
    if on_path:
        return Path(on_path)
    fallback = REPO_ROOT / "target" / "debug" / "dec"
    if fallback.exists():
        return fallback
    sys.stderr.write(
        "bootstrap_catalog: could not find the `dec` binary.\n"
        "  Set DEC_BINARY=/path/to/dec, or build it (cargo build -p decision-cli).\n"
    )
    sys.exit(1)


def invoke_dec(dec_bin: Path, workdir: Path, args: argparse.Namespace) -> int:
    cmd = [
        str(dec_bin),
        "--workdir",
        str(workdir),
        "_bootstrap-catalog",
        "--capabilities",
        str(args.capabilities.resolve()),
        "--bindings",
        str(args.bindings.resolve()),
    ]
    if args.migrate:
        cmd.append("--migrate")
    if args.dry_run:
        cmd.append("--dry-run")
    proc = subprocess.run(cmd, check=False)
    return proc.returncode


if __name__ == "__main__":
    sys.exit(main())
