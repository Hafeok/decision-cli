#!/usr/bin/env bash
# TC-213 — dec verify bench list returns post-migration BNCH-NNN ids
#
# Spec: .product/tests/TC-213-*.md
# Implements: FT-112 (ENV-to-BNCH rename)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# Build the binary
cargo build --quiet --package decision-cli --bin dec

DEC="$REPO_ROOT/target/debug/dec"
WORKDIR="$(mktemp -d --tmpdir tc-213.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

# Initialize workspace
"$DEC" init --template engineering-development >/dev/null 2>&1

# --- 1. Create two benches with BNCH ids -----------------------------------------
"$DEC" verify bench new \
  --id BNCH-099 \
  --type ephemeral-tempdir \
  --safety-class isolated \
  --allowed-ops shell,filesystem >/dev/null 2>&1

"$DEC" verify bench new \
  --id BNCH-100 \
  --type ephemeral-tempdir \
  --safety-class isolated \
  --allowed-ops shell,filesystem >/dev/null 2>&1

# --- 2. List benches and verify BNCH-NNN ids are returned ------------------------
output=$("$DEC" verify bench list)

if ! echo "$output" | grep -q "BNCH-099"; then
  echo "TC-213 FAIL: BNCH-099 not found in bench list" >&2
  echo "Output was:" >&2
  echo "$output" >&2
  exit 1
fi

if ! echo "$output" | grep -q "BNCH-100"; then
  echo "TC-213 FAIL: BNCH-100 not found in bench list" >&2
  echo "Output was:" >&2
  echo "$output" >&2
  exit 1
fi

# --- 3. Verify no old ENV-NNN ids appear ------------------------------------------
if echo "$output" | grep -q "ENV-001\|ENV-002"; then
  echo "TC-213 FAIL: Found old ENV-NNN id in bench list" >&2
  echo "Output was:" >&2
  echo "$output" >&2
  exit 1
fi

echo "TC-213 PASS: bench list returns BNCH-NNN ids"
exit 0
