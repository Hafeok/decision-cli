#!/usr/bin/env bash
# TC-228 — Re-running dec init on bootstrapped repo is idempotent and does not
#          corrupt store (invariant).
#
# Spec: .product/tests/TC-228-*.md
# Implements: FT-114 (dec init idempotency).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# Build the binary (debug profile is fine for tests).
cargo build --quiet --package decision-cli --bin dec

DEC="$REPO_ROOT/target/debug/dec"
WORKDIR="$(mktemp -d --tmpdir tc-228.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

# Set up a minimal .product/ structure
mkdir -p .product/tests
cat > .product/product.toml <<'EOF'
name = "tc-228-test-repo"
version = "0.1.0"
EOF

# Create a simple TC
cat > .product/tests/TC-001-example.md <<'EOF'
---
id: TC-001
title: Example test
type: invariant
status: unimplemented
phase: 1
validates:
  features: []
  adrs: []
runner: cargo-test
runner-args: "example"
runner-timeout: 30s
---

# TC-001 — Example test

## Description
Example test criterion.

## Acceptance Criteria
Passes.
EOF

# --- First init --------------------------------------------------------------
echo "==> Running dec init first time..."
if ! "$DEC" init --yes 2>&1 | tee init-1.log; then
  echo "TC-228 FAIL: First dec init exited non-zero" >&2
  cat init-1.log >&2
  exit 1
fi

# Capture initial state
if [ ! -f .dec/streams/tc-228-test-repo-development.ttl ]; then
  echo "TC-228 FAIL: Generated stream file not found after first init" >&2
  ls -la .dec/streams/ >&2 || echo ".dec/streams/ doesn't exist"
  exit 1
fi

INITIAL_TTL_SIZE=$(wc -c < .dec/streams/tc-228-test-repo-development.ttl)

# --- Add marker triple to store ---------------------------------------------
echo "<urn:tc-228-marker> <urn:p> \"1\" ." >> .dec/store/orchestration.nq

# --- Second init (should be idempotent) -------------------------------------
echo "==> Running dec init second time..."
if ! "$DEC" init --yes 2>&1 | tee init-2.log; then
  echo "TC-228 FAIL: Second dec init exited non-zero" >&2
  cat init-2.log >&2
  exit 1
fi

# Verify marker triple still exists
if ! grep -q "urn:tc-228-marker" .dec/store/orchestration.nq; then
  echo "TC-228 FAIL: Marker triple was lost - init wiped the store" >&2
  exit 1
fi

# Verify TTL size unchanged (deterministic regeneration)
SECOND_TTL_SIZE=$(wc -c < .dec/streams/tc-228-test-repo-development.ttl)
if [ "$INITIAL_TTL_SIZE" != "$SECOND_TTL_SIZE" ]; then
  echo "TC-228 FAIL: Stream file size changed ($INITIAL_TTL_SIZE -> $SECOND_TTL_SIZE) despite no .product/ changes" >&2
  exit 1
fi

# --- Add new TC with new runner type ----------------------------------------
cat > .product/tests/TC-002-new.md <<'EOF'
---
id: TC-002
title: New test
type: scenario
status: unimplemented
phase: 1
validates:
  features: []
  adrs: []
runner: pytest
runner-args: "test_example.py"
runner-timeout: 30s
---

# TC-002 — New test

## Description
New test with different runner.

## Acceptance Criteria
Passes.
EOF

# --- Third init (should pick up new runner) ---------------------------------
echo "==> Running dec init third time with new TC..."
if ! "$DEC" init --yes 2>&1 | tee init-3.log; then
  echo "TC-228 FAIL: Third dec init exited non-zero" >&2
  cat init-3.log >&2
  exit 1
fi

# Verify marker triple STILL exists
if ! grep -q "urn:tc-228-marker" .dec/store/orchestration.nq; then
  echo "TC-228 FAIL: Marker triple was lost on re-init with new TC" >&2
  exit 1
fi

# Verify TTL size changed (new subscription added)
THIRD_TTL_SIZE=$(wc -c < .dec/streams/tc-228-test-repo-development.ttl)
if [ "$INITIAL_TTL_SIZE" == "$THIRD_TTL_SIZE" ]; then
  echo "TC-228 FAIL: Stream file size unchanged despite new runner being added" >&2
  exit 1
fi

# Success
echo "TC-228 PASS: dec init is idempotent and preserves store state"
exit 0
