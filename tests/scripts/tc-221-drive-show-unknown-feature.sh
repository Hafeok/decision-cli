#!/usr/bin/env bash
# TC-221 — Unknown feature ID exits non-zero before any SPARQL query runs.
#
# Tests that `dec drive show FT-999` against a feature that doesn't exist
# in the product graph fails fast with a clear error message, while
# `dec drive show FT-X` against a known-but-undriven feature shows the
# empty-state paragraph (not an error).

set -euo pipefail

cd "$(dirname "$0")/../.." || exit 2

# Create temp workspace
TEMP_DIR=$(mktemp -d)
trap 'rm -rf "$TEMP_DIR"' EXIT

# Set up a minimal .product workspace with one feature
mkdir -p "$TEMP_DIR/.product/features"
cat > "$TEMP_DIR/.product/features/FT-X-test-feature.md" <<'EOF'
---
id: FT-X
title: Test Feature
status: draft
phase: 1
---

# FT-X — Test Feature

A test feature for TC-221.
EOF

# Test 1: Unknown feature returns non-zero exit code
echo "TEST 1: Unknown feature FT-999 should exit non-zero"
if ./target/debug/dec --workdir "$TEMP_DIR" drive show FT-999 2>"$TEMP_DIR/stderr.txt"; then
    echo "FAIL: dec drive show FT-999 should have exited non-zero"
    exit 1
fi

# Test 2: Stderr should contain "Unknown feature FT-999"
echo "TEST 2: Stderr should contain 'Unknown feature FT-999'"
if ! grep -q "Unknown feature FT-999" "$TEMP_DIR/stderr.txt"; then
    echo "FAIL: stderr should contain 'Unknown feature FT-999'"
    cat "$TEMP_DIR/stderr.txt"
    exit 1
fi

# Test 3: Stderr should hint at product feature list
echo "TEST 3: Stderr should hint at 'product feature list'"
if ! grep -q "product feature list" "$TEMP_DIR/stderr.txt"; then
    echo "FAIL: stderr should hint at 'product feature list'"
    cat "$TEMP_DIR/stderr.txt"
    exit 1
fi

# Test 4: Known feature with no drives should exit zero (empty-state path)
echo "TEST 4: Known feature FT-X with no drives should exit zero"
if ! ./target/debug/dec --workdir "$TEMP_DIR" drive show FT-X >"$TEMP_DIR/stdout.txt" 2>"$TEMP_DIR/stderr2.txt"; then
    echo "FAIL: dec drive show FT-X should have exited zero"
    cat "$TEMP_DIR/stderr2.txt"
    exit 1
fi

# Test 5: Stdout should contain empty-state paragraph (not error)
echo "TEST 5: Stdout should contain empty-state paragraph"
if ! grep -q "No drive history" "$TEMP_DIR/stdout.txt"; then
    echo "FAIL: stdout should contain 'No drive history'"
    cat "$TEMP_DIR/stdout.txt"
    exit 1
fi

if ! grep -q "FT-X" "$TEMP_DIR/stdout.txt"; then
    echo "FAIL: stdout should contain feature ID 'FT-X'"
    cat "$TEMP_DIR/stdout.txt"
    exit 1
fi

if ! grep -q "dec drive ship FT-X" "$TEMP_DIR/stdout.txt"; then
    echo "FAIL: stdout should suggest 'dec drive ship FT-X'"
    cat "$TEMP_DIR/stdout.txt"
    exit 1
fi

# Test 6: Stdout for known feature should NOT contain error about unknown feature
echo "TEST 6: Stdout should not contain 'Unknown feature' error"
if grep -q "Unknown feature" "$TEMP_DIR/stdout.txt"; then
    echo "FAIL: stdout should not contain 'Unknown feature' (this is the empty-state path, not error)"
    cat "$TEMP_DIR/stdout.txt"
    exit 1
fi

echo "All TC-221 tests passed"
exit 0
