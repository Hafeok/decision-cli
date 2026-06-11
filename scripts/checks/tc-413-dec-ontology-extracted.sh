#!/usr/bin/env bash
# scripts/checks/tc-413-dec-ontology-extracted.sh
#
# Exit criteria for FT-167 (ADR-086): the dec-ontology crate exists, is pure
# (dec-ontology-purity.sh passes hard, not the pre-migration warning), the
# crate compiles standalone, and decision-cli re-exports it as the
# core::ontology / core::vocab facades so feature-slice imports are unchanged.
#
# Exit 0: all criteria hold. Exit 1: any criterion fails.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

if [ ! -f crates/dec-ontology/Cargo.toml ]; then
  echo "ERROR: crates/dec-ontology does not exist (FT-167 not implemented)"
  exit 1
fi

bash scripts/checks/dec-ontology-purity.sh || {
  echo "ERROR: dec-ontology purity check did not pass hard (FT-167 exit criteria require exit 0)"
  exit 1
}

if ! grep -rqE 'pub use dec_ontology' crates/decision-cli/src/core/; then
  echo "ERROR: decision-cli core does not re-export dec_ontology (facade missing)"
  exit 1
fi

cargo check -p dec-ontology --quiet || {
  echo "ERROR: cargo check -p dec-ontology failed"
  exit 1
}

echo "OK: dec-ontology extracted, pure, compiling, facades intact (FT-167)"
exit 0
