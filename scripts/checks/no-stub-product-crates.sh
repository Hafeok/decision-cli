#!/usr/bin/env bash
# TC-328: Verify stub product-cli and product-shim crates are deleted

set -euo pipefail

REPO_ROOT="${DEC_PROJECT_ROOT:-.}"

# Check that the directories don't exist
if [ -d "$REPO_ROOT/crates/product-cli" ]; then
    echo "FAIL: crates/product-cli/ still exists"
    exit 1
fi

if [ -d "$REPO_ROOT/crates/product-shim" ]; then
    echo "FAIL: crates/product-shim/ still exists"
    exit 1
fi

# Check workspace Cargo.toml members
if grep -q '"crates/product-cli"' "$REPO_ROOT/Cargo.toml"; then
    echo "FAIL: Cargo.toml still lists crates/product-cli in members"
    exit 1
fi

if grep -q '"crates/product-shim"' "$REPO_ROOT/Cargo.toml"; then
    echo "FAIL: Cargo.toml still lists crates/product-shim in members"
    exit 1
fi

# Check that workspace.dependencies no longer has product-cli path entry
if grep -q 'product-cli.*path.*crates/product-cli' "$REPO_ROOT/Cargo.toml"; then
    echo "FAIL: Cargo.toml still has product-cli path dependency"
    exit 1
fi

# Check decision-cli Cargo.toml doesn't depend on product-cli workspace crate
if grep -q '^product-cli = { workspace = true }' "$REPO_ROOT/crates/decision-cli/Cargo.toml"; then
    echo "FAIL: decision-cli/Cargo.toml still depends on product-cli crate"
    exit 1
fi

echo "PASS: stub crates deleted and Cargo.toml updated"
exit 0
