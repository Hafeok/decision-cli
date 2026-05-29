#!/usr/bin/env bash
# TC-162 — graph_accepted_dispatch emits exactly one VerifyGraphRunDispatchEvent
#          per accepted graph and opens one Session.
#
# Spec: .product/tests/TC-162-*.md
# Implements: FT-100 (graph-accepted auto-dispatch).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-162.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"
"$DEC" init --template engineering-development >/dev/null 2>&1

# Configure the subscription
cat >>.dec/config.toml <<'EOF'

[verify_graph_runner.on_graph_accepted]
enabled = true
dedup_ttl_seconds = 300
EOF

# Create fixture feature
mkdir -p .product/features
cat >.product/features/FT-TC162.md <<'EOF'
---
id: FT-TC162
title: TC-162 fixture
tests: []
---
fixture
EOF

# --- Scenario A: single graph, single env (happy path) ----------------------
echo "TC-162: Scenario A — single graph, single env"

before_results=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)

# Author a minimal passing graph
"$DEC" verify graph new --id VG-200 --verifies FT-TC162 \
  --environment ENV-001-ephemeral-cli >/dev/null 2>&1

"$DEC" verify step add VG-200 --type shell-command \
  --field command="echo pass" \
  --field expect-exit-code=0 >/dev/null 2>&1

# Dispatch via the internal trigger
dispatch_out=$("$DEC" _dispatch graph-accepted VG-200 2>&1)
if [ $? -ne 0 ]; then
  echo "TC-162 FAIL Scenario A: _dispatch exited non-zero" >&2
  echo "$dispatch_out" >&2
  exit 1
fi

# Verify dispatch reported success
if ! printf '%s' "$dispatch_out" | grep -q 'dispatched=1'; then
  echo "TC-162 FAIL Scenario A: dispatch output did not report dispatched=1:" >&2
  echo "$dispatch_out" >&2
  exit 1
fi

# Verify a session IRI was emitted
if ! printf '%s' "$dispatch_out" | grep -q 'session=urn:dec:session/verify-graph-runner'; then
  echo "TC-162 FAIL Scenario A: dispatch output did not contain session IRI:" >&2
  echo "$dispatch_out" >&2
  exit 1
fi

# Check that exactly one result artifact was created
after_results=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)
new_results=$((after_results - before_results))
if [ "$new_results" -ne 1 ]; then
  echo "TC-162 FAIL Scenario A: expected 1 new result artifact, got $new_results" >&2
  exit 1
fi

echo "TC-162: Scenario A PASS"

# --- Scenario B: config enabled=false suppresses dispatch -------------------
echo "TC-162: Scenario B — disabled config"

# Disable the subscription
cat >.dec/config.toml <<'EOF'
[verify_graph_runner.on_graph_accepted]
enabled = false
EOF

before_results=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)

# Author another graph
"$DEC" verify graph new --id VG-201 --verifies FT-TC162 \
  --environment ENV-001-ephemeral-cli >/dev/null 2>&1

"$DEC" verify step add VG-201 --type shell-command \
  --field command="echo pass" \
  --field expect-exit-code=0 >/dev/null 2>&1

# Dispatch — should report disabled
dispatch_out=$("$DEC" _dispatch graph-accepted VG-201 2>&1)
if [ $? -ne 0 ]; then
  echo "TC-162 FAIL Scenario B: _dispatch exited non-zero" >&2
  exit 1
fi

if ! printf '%s' "$dispatch_out" | grep -q 'disabled'; then
  echo "TC-162 FAIL Scenario B: dispatch did not report disabled:" >&2
  echo "$dispatch_out" >&2
  exit 1
fi

# Verify no new result was created
after_results=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)
new_results=$((after_results - before_results))
if [ "$new_results" -ne 0 ]; then
  echo "TC-162 FAIL Scenario B: expected 0 new results, got $new_results" >&2
  exit 1
fi

echo "TC-162: Scenario B PASS"

# --- Scenario C: dedup ledger prevents re-dispatch --------------------------
echo "TC-162: Scenario C — dedup ledger"

# Re-enable
cat >.dec/config.toml <<'EOF'
[verify_graph_runner.on_graph_accepted]
enabled = true
dedup_ttl_seconds = 300
EOF

# Author and dispatch
"$DEC" verify graph new --id VG-202 --verifies FT-TC162 \
  --environment ENV-001-ephemeral-cli >/dev/null 2>&1

"$DEC" verify step add VG-202 --type shell-command \
  --field command="echo pass" \
  --field expect-exit-code=0 >/dev/null 2>&1

before_results=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)

# First dispatch
dispatch_out1=$("$DEC" _dispatch graph-accepted VG-202 2>&1)
if ! printf '%s' "$dispatch_out1" | grep -q 'dispatched=1'; then
  echo "TC-162 FAIL Scenario C: first dispatch should report dispatched=1" >&2
  exit 1
fi

# Verify ledger entry exists
ledger_out=$("$DEC" _dispatch ledger-graph-accepted VG-202 ENV-001-ephemeral-cli 2>&1)
if ! printf '%s' "$ledger_out" | grep -q 'entry graph='; then
  echo "TC-162 FAIL Scenario C: ledger entry not found after first dispatch:" >&2
  echo "$ledger_out" >&2
  exit 1
fi

# Second dispatch within TTL should report skipped
dispatch_out2=$("$DEC" _dispatch graph-accepted VG-202 2>&1)
if ! printf '%s' "$dispatch_out2" | grep -q 'skipped_dedup=1'; then
  echo "TC-162 FAIL Scenario C: second dispatch should report skipped_dedup=1" >&2
  echo "$dispatch_out2" >&2
  exit 1
fi

after_results=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)
new_results=$((after_results - before_results))

# Should be exactly 1 (not 2) because the second was dedup-skipped
if [ "$new_results" -ne 1 ]; then
  echo "TC-162 FAIL Scenario C: expected exactly 1 result after dedup, got $new_results" >&2
  exit 1
fi

echo "TC-162: Scenario C PASS"

# --- Scenario D: missing env produces error ---------------------------------
echo "TC-162: Scenario D — missing env"

# Author graph referencing non-existent env
"$DEC" verify graph new --id VG-203 --verifies FT-TC162 \
  --environment ENV-DOES-NOT-EXIST >/dev/null 2>&1 || true

"$DEC" verify step add VG-203 --type shell-command \
  --field command="echo pass" \
  --field expect-exit-code=0 >/dev/null 2>&1 || true

before_results=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)

# Dispatch should succeed but report env_errors
dispatch_out=$("$DEC" _dispatch graph-accepted VG-203 2>&1 || true)

if ! printf '%s' "$dispatch_out" | grep -E 'env_errors=1|env-error|env catalog entry missing' >/dev/null; then
  echo "TC-162 FAIL Scenario D: expected env_errors in output, got:" >&2
  echo "$dispatch_out" >&2
  exit 1
fi

# No new result should be created
after_results=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)
new_results=$((after_results - before_results))
if [ "$new_results" -ne 0 ]; then
  echo "TC-162 FAIL Scenario D: expected 0 new results for missing env, got $new_results" >&2
  exit 1
fi

echo "TC-162: Scenario D PASS"
echo "TC-162 PASS"
