#!/usr/bin/env bash
# scripts/checks/tc-417-dec-graph-tests.sh
#
# Exit criteria for FT-168 (ADR-086): the store, GraphWriter-chokepoint,
# and store-aware SHACL tests that moved with the graph-access layer pass
# unmodified in their new home, `cargo test -p dec-graph`.
set -euo pipefail
if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi
if [ ! -f crates/dec-graph/Cargo.toml ]; then
  echo "ERROR: crates/dec-graph does not exist (FT-168 not implemented)"
  exit 1
fi
cargo test -p dec-graph --quiet
