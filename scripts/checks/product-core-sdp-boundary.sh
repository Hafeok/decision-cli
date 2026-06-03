#!/usr/bin/env bash
# TC-331: Verify product-core has no dependency on decision-cli or oxi-events

set -euo pipefail

REPO_ROOT="${DEC_PROJECT_ROOT:-.}"

# Fetch dependencies to ensure product-core is available locally
cd "$REPO_ROOT"
cargo fetch --locked >/dev/null 2>&1 || true

# Find the fetched product-core Cargo.toml
PRODUCT_CORE_TOML=$(find ~/.cargo/git/checkouts/product-cli-* -name "Cargo.toml" -path "*/product-core/*" 2>/dev/null | head -1)

if [ -z "$PRODUCT_CORE_TOML" ]; then
    echo "FAIL: Could not locate product-core Cargo.toml"
    exit 2
fi

# Check that product-core doesn't depend on decision-cli or oxi-events
if grep -E '^(decision-cli|oxi-events)\s*=' "$PRODUCT_CORE_TOML" 2>/dev/null; then
    echo "FAIL: product-core Cargo.toml contains forbidden dependency on decision-cli or oxi-events"
    exit 1
fi

echo "PASS: product-core SDP boundary verified"
exit 0
