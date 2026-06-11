#!/usr/bin/env bash
# scripts/checks/dec-ontology-purity.sh
#
# Enforces ADR-086's dec-ontology contract: the domain crate at the center of
# the workspace is pure data. Its manifest may declare only RDF model types
# and serialization/error/time/id crates. It must not declare a store, an
# async runtime, HTTP, CLI, or any workspace crate:
#
#   forbidden: oxigraph, tokio, axum, reqwest, clap, anyhow, oxi-events,
#              decision-cli, product-core, dec-graph, dec-harness
#
# Exit 0: dec-ontology is pure.
# Exit 1: a forbidden dependency is declared.
# Exit 2: crates/dec-ontology does not exist yet (pre-FT-167 migration);
#         warning per the ADR-013 runner contract.
#
# Diagnostic output goes to stdout so `product verify` captures it.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

CARGO_TOML="crates/dec-ontology/Cargo.toml"
if [ ! -f "$CARGO_TOML" ]; then
  echo "WARN: $CARGO_TOML not extracted yet (FT-167 pending); purity not yet binding"
  exit 2
fi

FAILED=0
FORBIDDEN="oxigraph tokio axum reqwest clap anyhow oxi-events decision-cli product-core dec-graph dec-harness"

for dep in $FORBIDDEN; do
  if grep -qE "^\s*${dep//-/[-_]}\s*[=.]" "$CARGO_TOML"; then
    echo "ERROR: $CARGO_TOML declares forbidden dependency '$dep' (ADR-086 violation)"
    FAILED=1
  fi
done

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

echo "OK: dec-ontology dependency tree is pure (ADR-086)"
exit 0
