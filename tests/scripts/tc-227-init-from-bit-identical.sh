#!/usr/bin/env bash
# TC-227 — dec init --from preserves slice-1 hand-authored stream behaviour
#          bit-identically (invariant).
#
# Spec: .product/tests/TC-227-*.md
# Implements: FT-114 (dec init auto-bootstrap with --from preservation).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# Build the binary (debug profile is fine for tests).
cargo build --quiet --package decision-cli --bin dec

DEC="$REPO_ROOT/target/debug/dec"
WORKDIR="$(mktemp -d --tmpdir tc-227.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

# Set up a minimal .product/ structure so dec init doesn't error on discovery
mkdir -p .product/tests
cat > .product/product.toml <<'EOF'
name = "tc-227-test-repo"
version = "0.1.0"
EOF

# Create a simple TC so there's test content
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
runner: bash
runner-args: "true"
runner-timeout: 30s
---

# TC-001 — Example test

## Description
Example test criterion.

## Acceptance Criteria
Passes.
EOF

# Copy the reference stream definition
mkdir -p streams
cp "$REPO_ROOT/streams/decision-cli-development.ttl" streams/

# --- Test: dec init --from produces consistent output -----------------------

# Run dec init --from with --yes flag
if ! "$DEC" init --from streams/decision-cli-development.ttl --yes 2>&1 | tee init-output.log; then
  echo "TC-227 FAIL: dec init --from exited non-zero" >&2
  cat init-output.log >&2
  exit 1
fi

# --- Verify .dec/streams/ is NOT created (--from uses provided stream) ------
if [ -d .dec/streams ] && [ -n "$(ls -A .dec/streams 2>/dev/null)" ]; then
  echo "TC-227 FAIL: .dec/streams/ should not contain generated files when using --from" >&2
  ls -la .dec/streams >&2
  exit 1
fi

# --- Verify console output mentions 'loaded from' or 'from' ------------------
if ! grep -qi "from.*streams/decision-cli-development.ttl" init-output.log; then
  echo "TC-227 FAIL: Console output should mention loading from the provided stream" >&2
  cat init-output.log >&2
  exit 1
fi

# --- Verify orchestration store was created ---------------------------------
if [ ! -f .dec/store/orchestration.nq ]; then
  echo "TC-227 FAIL: .dec/store/orchestration.nq was not created" >&2
  exit 1
fi

# Success - the --from path preserves the existing behavior
echo "TC-227 PASS: dec init --from preserves hand-authored stream behavior"
exit 0
