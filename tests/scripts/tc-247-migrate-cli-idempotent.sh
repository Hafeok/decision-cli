#!/usr/bin/env bash
# TC-247 — Migration tool is idempotent against an already-migrated
# store and reports zero rewrites.
#
# Spec: .product/tests/TC-247-*.md
# Implements: FT-117

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-247.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT
mkdir -p "$WORKDIR/.dec/store"

# Seed a workdir with ENV vocab.
cat > "$WORKDIR/.dec/store/orchestration.nq" <<'EOF'
<https://decision-cli.dev/ns/env/ENV-002> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationEnvironment> <urn:graph:test> .
EOF

# First migration: should rewrite.
out1=$("$DEC" --workdir "$WORKDIR" migrate env-to-bench)
if ! echo "$out1" | grep -q "1 class"; then
  echo "TC-247 FAIL: expected first migration to report 1 class rewrite, got: $out1" >&2
  exit 1
fi

# Snapshot the store after first migration.
cp "$WORKDIR/.dec/store/orchestration.nq" /tmp/tc-247-snap1.nq

# Second migration: should be no-op.
out2=$("$DEC" --workdir "$WORKDIR" migrate env-to-bench)
if ! echo "$out2" | grep -q "no-op"; then
  echo "TC-247 FAIL: expected second migration to report no-op, got: $out2" >&2
  exit 1
fi

# Snapshot after second migration; assert byte-identical to first.
if ! diff -q /tmp/tc-247-snap1.nq "$WORKDIR/.dec/store/orchestration.nq" >/dev/null; then
  echo "TC-247 FAIL: store changed between idempotent migrations" >&2
  diff /tmp/tc-247-snap1.nq "$WORKDIR/.dec/store/orchestration.nq" >&2
  exit 1
fi
rm -f /tmp/tc-247-snap1.nq

# Dry-run on already-migrated store: should also report no-op + DRY-RUN marker.
out3=$("$DEC" --workdir "$WORKDIR" migrate env-to-bench --dry-run)
if ! echo "$out3" | grep -q "DRY-RUN"; then
  echo "TC-247 FAIL: dry-run output missing DRY-RUN marker, got: $out3" >&2
  exit 1
fi
if ! echo "$out3" | grep -q "no-op"; then
  echo "TC-247 FAIL: dry-run should report no-op for already-migrated, got: $out3" >&2
  exit 1
fi

echo "TC-247 PASS: migration is idempotent; dry-run honours no-op state"
exit 0
