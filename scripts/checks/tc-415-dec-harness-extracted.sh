#!/usr/bin/env bash
# scripts/checks/tc-415-dec-harness-extracted.sh
#
# Exit criteria for FT-169 (ADR-086): the dec-harness crate exists, sits on
# dec-graph + dec-ontology, the full crate topology check passes hard, the
# crate compiles standalone, and decision-cli re-exports it as core facades.
#
# Exit 0: all criteria hold. Exit 1: any criterion fails.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

if [ ! -f crates/dec-harness/Cargo.toml ]; then
  echo "ERROR: crates/dec-harness does not exist (FT-169 not implemented)"
  exit 1
fi

for dep in dec-graph dec-ontology; do
  if ! grep -qE "^\s*${dep//-/[-_]}\s*[=.]" crates/dec-harness/Cargo.toml; then
    echo "ERROR: dec-harness does not depend on $dep (ADR-086 expects the stable layers beneath it)"
    exit 1
  fi
done

bash scripts/checks/crate-dependency-topology.sh || {
  echo "ERROR: crate topology check did not pass hard (FT-169 exit criteria require exit 0)"
  exit 1
}

if ! grep -rqE 'pub use dec_harness' crates/decision-cli/src/core/; then
  echo "ERROR: decision-cli core does not re-export dec_harness (facade missing)"
  exit 1
fi

cargo check -p dec-harness --quiet || {
  echo "ERROR: cargo check -p dec-harness failed"
  exit 1
}

echo "OK: dec-harness extracted, topology intact, compiling, facades intact (FT-169)"
exit 0
