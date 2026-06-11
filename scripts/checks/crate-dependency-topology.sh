#!/usr/bin/env bash
# scripts/checks/crate-dependency-topology.sh
#
# Enforces ADR-086 stable-dependency crate topology:
#
#       oxrdf <- dec-ontology <- dec-graph <- dec-harness <- decision-cli
#
#   - no workspace crate depends on decision-cli
#   - dec-ontology depends on no workspace crate
#   - dec-graph does not depend on dec-harness or decision-cli, and not on clap
#   - dec-harness does not depend on decision-cli, and not on clap
#
# Exit 0: topology intact.
# Exit 1: a forbidden dependency edge was found.
# Exit 2: the extracted crates do not exist yet (pre-FT-167/168/169 migration);
#         the constraint is not yet binding. Warning per the ADR-013 runner
#         contract so `product verify --platform` surfaces it without blocking.
#
# Diagnostic output goes to stdout so `product verify` captures it.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

FAILED=0

# manifest <crate-name> -> path, empty if the crate does not exist yet
manifest() {
  local m="crates/$1/Cargo.toml"
  [ -f "$m" ] && echo "$m" || true
}

# forbid <manifest> <dep-name>
forbid() {
  local m="$1" dep="$2"
  if grep -qE "^\s*${dep//-/[-_]}\s*[=.]" "$m"; then
    echo "ERROR: $m depends on $dep (ADR-086 violation)"
    FAILED=1
  fi
}

ONTOLOGY="$(manifest dec-ontology)"
GRAPH="$(manifest dec-graph)"
HARNESS="$(manifest dec-harness)"

if [ -z "$ONTOLOGY" ] && [ -z "$GRAPH" ] && [ -z "$HARNESS" ]; then
  echo "WARN: dec-ontology/dec-graph/dec-harness not extracted yet (FT-167..FT-169 pending); topology not yet binding"
  exit 2
fi

# Nothing in the workspace may depend on the binary crate.
for m in crates/*/Cargo.toml; do
  [ "$m" = "crates/decision-cli/Cargo.toml" ] && continue
  forbid "$m" "decision-cli"
done

if [ -n "$ONTOLOGY" ]; then
  for dep in dec-graph dec-harness oxi-events product-core; do
    forbid "$ONTOLOGY" "$dep"
  done
fi

if [ -n "$GRAPH" ]; then
  for dep in dec-harness clap; do
    forbid "$GRAPH" "$dep"
  done
fi

if [ -n "$HARNESS" ]; then
  forbid "$HARNESS" "clap"
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

echo "OK: ADR-086 crate dependency topology is intact"
exit 0
