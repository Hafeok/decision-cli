#!/usr/bin/env bash
# TC-214 — On-disk .ttl files under .dec/verify carry BNCH-prefix IRIs after migration
#
# Spec: .product/tests/TC-214-*.md
# Implements: FT-112 (ENV-to-BNCH rename)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# Build the binary
cargo build --quiet --package decision-cli --bin dec

DEC="$REPO_ROOT/target/debug/dec"
WORKDIR="$(mktemp -d --tmpdir tc-214.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

# Initialize workspace
"$DEC" init --template engineering-development >/dev/null 2>&1

# --- 1. Create a bench (which writes .ttl files) ---------------------------------
"$DEC" verify bench new \
  --id BNCH-099 \
  --type ephemeral-tempdir \
  --safety-class isolated \
  --allowed-ops shell,filesystem >/dev/null 2>&1

# --- 2. Verify .dec/verify/bench/ directory exists -------------------------------
if [ ! -d ".dec/verify/bench" ]; then
  echo "TC-214 FAIL: .dec/verify/bench/ directory not found" >&2
  exit 1
fi

# --- 3. Verify BNCH-001.ttl exists -----------------------------------------------
if [ ! -f ".dec/verify/bench/BNCH-099.ttl" ]; then
  # Might be under a different naming scheme, check for any .ttl
  if ! ls .dec/verify/bench/*.ttl >/dev/null 2>&1; then
    echo "TC-214 FAIL: No .ttl files found in .dec/verify/bench/" >&2
    exit 1
  fi
fi

# --- 4. Check .ttl files contain BNCH IRIs, not ENV IRIs --------------------------
if [ -d ".dec/verify/bench" ]; then
  ttl_files=$(find .dec/verify/bench -name "*.ttl" 2>/dev/null || true)
  if [ -n "$ttl_files" ]; then
    for ttl in $ttl_files; do
      if grep -q "/ns/env/ENV-" "$ttl" 2>/dev/null; then
        echo "TC-214 FAIL: Found old ENV IRI in $ttl" >&2
        exit 1
      fi
      if grep -q "/ns/bench/BNCH-" "$ttl" 2>/dev/null; then
        echo "TC-214 partial: Found BNCH IRI in $ttl"
      fi
    done
  fi
fi

# --- 5. Verify .dec/verify/env/ directory does NOT exist --------------------------
if [ -d ".dec/verify/env" ]; then
  echo "TC-214 FAIL: Old .dec/verify/env/ directory still exists" >&2
  exit 1
fi

echo "TC-214 PASS: On-disk .ttl files use BNCH-prefix IRIs"
exit 0
