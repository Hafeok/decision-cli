#!/usr/bin/env bash
# TC-209 — ENV-prefix IRIs are absent from all source files after rename
#
# Spec: .product/tests/TC-209-*.md
# Implements: FT-112 (ENV-to-BNCH rename)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# --- Search for old ENV IRI strings ----------------------------------------------
# These should all be zero matches after the rename

OLD_IRIS=(
  "https://decision-cli\.dev/ns/env/"
  "https://decision-cli\.dev/ns#envType"
  "https://decision-cli\.dev/ns#VerificationEnvironment"
  "https://decision-cli\.dev/ns#ranInEnvironment"
  "https://decision-cli\.dev/ns/graph/verify-env"
)

SEARCH_DIRS=(
  "crates/"
  "tests/"
  "docs/"
)

for iri_pattern in "${OLD_IRIS[@]}"; do
  for dir in "${SEARCH_DIRS[@]}"; do
    # Exclude the migration test file which legitimately references old IRIs
    # Exclude auto_dispatch.rs which has ledgerEnvironment (different axis)
    # Exclude test script files which may reference old IRIs in comments
    if grep -rn "$iri_pattern" "$dir" 2>/dev/null | \
       grep -v "ft_112_migration.rs" | \
       grep -v "auto_dispatch.rs" | \
       grep -v "ledgerEnvironment" | \
       grep -v "IRI_DEC_LEDGER_ENVIRONMENT" | \
       grep -v "tests/scripts/tc-"; then
      echo "TC-209 FAIL: Found old ENV IRI pattern '$iri_pattern' in $dir" >&2
      exit 1
    fi
  done
done

echo "TC-209 PASS: No ENV-prefix IRIs found in source files"
exit 0
