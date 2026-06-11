#!/usr/bin/env bash
# scripts/checks/tc-414-dec-graph-extracted.sh
#
# Exit criteria for FT-168 (ADR-086): the dec-graph crate exists, the crate
# topology check passes hard (exit 0, not the pre-migration warning), the
# crate compiles standalone, and decision-cli re-exports it as core facades
# so feature-slice imports are unchanged.
#
# Exit 0: all criteria hold. Exit 1: any criterion fails.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

if [ ! -f crates/dec-graph/Cargo.toml ]; then
  echo "ERROR: crates/dec-graph does not exist (FT-168 not implemented)"
  exit 1
fi

if ! grep -qE '^\s*dec[-_]ontology\s*[=.]' crates/dec-graph/Cargo.toml; then
  echo "ERROR: dec-graph does not depend on dec-ontology (ADR-086 expects the domain beneath it)"
  exit 1
fi

bash scripts/checks/crate-dependency-topology.sh || {
  echo "ERROR: crate topology check did not pass hard (FT-168 exit criteria require exit 0)"
  exit 1
}

if ! grep -rqE 'pub use dec_graph' crates/decision-cli/src/core/; then
  echo "ERROR: decision-cli core does not re-export dec_graph (facade missing)"
  exit 1
fi

cargo check -p dec-graph --quiet || {
  echo "ERROR: cargo check -p dec-graph failed"
  exit 1
}

echo "OK: dec-graph extracted, topology intact, compiling, facades intact (FT-168)"
exit 0
