#!/usr/bin/env bash
# TC-208 — Vocab module rename: verify_env -> verify_bench compiles with renamed identifiers
#
# Spec: .product/tests/TC-208-*.md
# Implements: FT-112 (ENV-to-BNCH rename)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# --- 1. cargo build --workspace must exit 0 -------------------------------------
if ! cargo build --workspace --quiet 2>&1; then
  echo "TC-208 FAIL: cargo build --workspace failed" >&2
  exit 1
fi

# --- 2. Old constant names must be absent ----------------------------------------
# Grep for old ENV constant names (should be zero matches)
OLD_CONSTANTS=(
  "IRI_DEC_ENV_PREFIX"
  "IRI_DEC_VERIFICATION_ENVIRONMENT"
  "IRI_DEC_ENV_TYPE"
  "IRI_DEC_GRAPH_VERIFY_ENV"
  "IRI_DEC_RAN_IN_ENVIRONMENT"
)

for const in "${OLD_CONSTANTS[@]}"; do
  if grep -rn "$const" crates/decision-cli/src/ 2>/dev/null | grep -v "IRI_DEC_LEDGER_ENVIRONMENT" | grep -v "auto_dispatch.rs"; then
    echo "TC-208 FAIL: Found old constant $const in source" >&2
    exit 1
  fi
done

# --- 3. New constant names must be present ---------------------------------------
# Grep for new BENCH constant names (should have at least 1 match each)
NEW_CONSTANTS=(
  "IRI_DEC_BENCH_PREFIX"
  "IRI_DEC_VERIFICATION_BENCH"
  "IRI_DEC_BENCH_TYPE"
  "IRI_DEC_GRAPH_VERIFY_BENCH"
  "IRI_DEC_RAN_ON_BENCH"
)

for const in "${NEW_CONSTANTS[@]}"; do
  if ! grep -rn "$const" crates/decision-cli/src/ >/dev/null 2>&1; then
    echo "TC-208 FAIL: New constant $const not found in source" >&2
    exit 1
  fi
done

echo "TC-208 PASS: Vocab module compiles with renamed identifiers"
exit 0
