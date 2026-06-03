#!/usr/bin/env bash
# TC-340: Verify workspace Cargo.toml declares product-core and product-mcp git deps

set -euo pipefail

REPO_ROOT="${DEC_PROJECT_ROOT:-.}"
CARGO_TOML="$REPO_ROOT/Cargo.toml"

# Check workspace.dependencies has product-core with git and rev
if ! grep -q 'product-core.*git.*"https://github.com/Hafeok/product-cli"' "$CARGO_TOML"; then
    echo "FAIL: product-core git dependency not found in workspace.dependencies"
    exit 1
fi

if ! grep -q 'product-core.*rev.*"5fad7aa11ca8787ff74e87bb00e1cc0bdfb8b2c1"' "$CARGO_TOML"; then
    echo "FAIL: product-core does not have the expected pinned rev"
    exit 1
fi

# Check workspace.dependencies has product-mcp with git and rev
if ! grep -q 'product-mcp.*git.*"https://github.com/Hafeok/product-cli"' "$CARGO_TOML"; then
    echo "FAIL: product-mcp git dependency not found in workspace.dependencies"
    exit 1
fi

if ! grep -q 'product-mcp.*rev.*"5fad7aa11ca8787ff74e87bb00e1cc0bdfb8b2c1"' "$CARGO_TOML"; then
    echo "FAIL: product-mcp does not have the expected pinned rev"
    exit 1
fi

# Check that workspace.members no longer includes stub crates
if grep -q '"crates/product-cli"' "$CARGO_TOML"; then
    echo "FAIL: workspace.members still includes crates/product-cli"
    exit 1
fi

if grep -q '"crates/product-shim"' "$CARGO_TOML"; then
    echo "FAIL: workspace.members still includes crates/product-shim"
    exit 1
fi

# Check decision-cli/Cargo.toml depends on product-core and product-mcp
DECISION_CLI_TOML="$REPO_ROOT/crates/decision-cli/Cargo.toml"
if ! grep -q '^product-core = { workspace = true }' "$DECISION_CLI_TOML"; then
    echo "FAIL: decision-cli does not depend on product-core"
    exit 1
fi

if ! grep -q '^product-mcp = { workspace = true }' "$DECISION_CLI_TOML"; then
    echo "FAIL: decision-cli does not depend on product-mcp"
    exit 1
fi

echo "PASS: workspace Cargo.toml shape verified"
exit 0
