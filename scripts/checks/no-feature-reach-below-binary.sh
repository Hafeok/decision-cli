#!/usr/bin/env bash
# scripts/checks/no-feature-reach-below-binary.sh
#
# Enforces ADR-086 + ADR-016 jointly: the crates below the binary
# (dec-ontology, dec-graph, dec-harness) must never reference the
# binary's feature slices. Cargo makes a real import impossible; this
# audit additionally catches doc links and commented code that would
# normalise the upward reach (the pattern that produced the pre-FT-169
# core->features imports in cluster_session and trace_writer).
#
# Exit 0: no references. Exit 1: a reference was found.
set -euo pipefail
if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi
HITS="$(grep -rn 'crate::features\|decision_cli::features' \
  crates/dec-ontology/src crates/dec-graph/src crates/dec-harness/src 2>/dev/null || true)"
if [ -n "$HITS" ]; then
  echo "ERROR: feature-slice references below the binary crate (ADR-086):"
  echo "$HITS" | sed 's/^/  /'
  exit 1
fi
echo "OK: no feature-slice reach below the binary crate"
exit 0
