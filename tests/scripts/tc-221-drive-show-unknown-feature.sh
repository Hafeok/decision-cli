#!/usr/bin/env bash
# TC-221 — Unknown feature ID exits non-zero before any SPARQL query runs (scenario).
#
# Tests that `dec drive show FT-999` against a feature that doesn't exist
# fails fast with a clear error message.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Create temp workspace
TEMP_DIR=$(mktemp -d)
trap 'rm -rf "$TEMP_DIR"' EXIT

cd "$TEMP_DIR"

# Initialize a minimal product graph with one feature
mkdir -p .product/features
cat > .product/features/FT-001-test-feature.md <<'EOF'
---
id: FT-001
title: Test Feature
status: in-progress
---

# Test Feature

A test feature for TC-221.
EOF

# Run dec drive show against unknown feature - should fail
if "$PROJECT_ROOT/target/debug/dec" drive show FT-999 2>stderr.txt; then
    echo "FAIL: dec drive show FT-999 should have failed but succeeded"
    exit 1
fi

# Check exit code was non-zero (captured by `if` statement above failing)
# Now check stderr contains the expected error message
if ! grep -q "Unknown feature FT-999" stderr.txt && ! grep -q "feature not found" stderr.txt; then
    echo "FAIL: stderr should contain 'Unknown feature' or 'feature not found' message"
    cat stderr.txt
    exit 1
fi

# Also check it mentions product feature list or similar helpful hint
if ! grep -q "product feature list" stderr.txt && ! grep -q "product" stderr.txt; then
    echo "WARN: stderr should ideally mention 'product feature list' but doesn't"
    # Not a hard failure - just warn
fi

# Now run against the known feature FT-001 (should succeed with exit 0, showing empty state)
if ! "$PROJECT_ROOT/target/debug/dec" drive show FT-001 >stdout.txt 2>stderr2.txt; then
    echo "FAIL: dec drive show FT-001 should have succeeded but failed"
    cat stderr2.txt
    exit 1
fi

# Should show the empty-state paragraph, not an error
if ! grep -q "No drive history" stdout.txt; then
    echo "FAIL: stdout should contain 'No drive history' for known-but-undriven feature"
    cat stdout.txt
    exit 1
fi

if grep -q "Unknown feature" stdout.txt || grep -q "Unknown feature" stderr2.txt; then
    echo "FAIL: Known feature FT-001 should not produce 'Unknown feature' error"
    cat stdout.txt
    cat stderr2.txt
    exit 1
fi

echo "TC-221 PASS: unknown feature fails with clear error, known-but-undriven succeeds with empty state"
