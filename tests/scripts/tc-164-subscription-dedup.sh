#!/usr/bin/env bash
# TC-164 — subscription dedup TTL coalesces rapid re-fires within the window
#          into a single dispatch.
#
# Spec: .product/tests/TC-164-*.md
# Implements: FT-100 (dedup TTL).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-164.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"
"$DEC" init --template engineering-development >/dev/null 2>&1

# Configure short TTL for testability
cat >>.dec/config.toml <<'EOF'

[verify_graph_runner.on_graph_accepted]
enabled = true
dedup_ttl_seconds = 5

[verify_graph_runner.on_code_change]
enabled = true
dedup_ttl_seconds = 5
EOF

# Create fixture features
mkdir -p .product/features
cat >.product/features/FT-TC164-A.md <<'EOF'
---
id: FT-TC164-A
title: TC-164 fixture A
tests: []
---
fixture
EOF

cat >.product/features/FT-TC164-B.md <<'EOF'
---
id: FT-TC164-B
title: TC-164 fixture B
tests: [TC-B-1]
---
fixture
EOF

# --- Scenario A: graph_accepted dedup window --------------------------------
echo "TC-164: Scenario A — graph_accepted dedup"

# Author a minimal graph
"$DEC" verify graph new --id VG-300 --verifies FT-TC164-A \
  --bench BNCH-001-ephemeral-cli >/dev/null 2>&1

"$DEC" verify step add VG-300 --type shell-command \
  --field command="echo pass" \
  --field expect-exit-code=0 >/dev/null 2>&1

before_results=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)

# First dispatch
out1=$("$DEC" _dispatch graph-accepted VG-300 2>&1)
if ! printf '%s' "$out1" | grep -q 'dispatched=1'; then
  echo "TC-164 FAIL Scenario A: first dispatch should report dispatched=1" >&2
  echo "$out1" >&2
  exit 1
fi

after_first=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)
first_delta=$((after_first - before_results))
if [ "$first_delta" -ne 1 ]; then
  echo "TC-164 FAIL Scenario A: first dispatch should create 1 result, got $first_delta" >&2
  exit 1
fi

# Second dispatch within TTL (within 2s)
sleep 1
out2=$("$DEC" _dispatch graph-accepted VG-300 2>&1)
if ! printf '%s' "$out2" | grep -q 'skipped_dedup=1'; then
  echo "TC-164 FAIL Scenario A: second dispatch within TTL should report skipped_dedup=1" >&2
  echo "$out2" >&2
  exit 1
fi

after_second=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)
second_delta=$((after_second - after_first))
if [ "$second_delta" -ne 0 ]; then
  echo "TC-164 FAIL Scenario A: second dispatch within TTL should be skipped, but created $second_delta results" >&2
  exit 1
fi

# Wait for TTL to expire (5s + 1s safety)
sleep 6

# Third dispatch after TTL should succeed
out3=$("$DEC" _dispatch graph-accepted VG-300 2>&1)
if ! printf '%s' "$out3" | grep -q 'dispatched=1'; then
  echo "TC-164 FAIL Scenario A: third dispatch after TTL should report dispatched=1" >&2
  echo "$out3" >&2
  exit 1
fi

after_third=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)
third_delta=$((after_third - after_second))
if [ "$third_delta" -ne 1 ]; then
  echo "TC-164 FAIL Scenario A: third dispatch after TTL should create 1 result, got $third_delta" >&2
  exit 1
fi

echo "TC-164: Scenario A PASS"

# --- Scenario B: code_change_committed dedup window -------------------------
echo "TC-164: Scenario B — code_change_committed dedup"

# Create a graph covering the fixture feature
"$DEC" verify graph new --id VG-301 --verifies FT-TC164-B \
  --bench BNCH-001-ephemeral-cli >/dev/null 2>&1

"$DEC" verify step add VG-301 --type shell-command \
  --field command="echo pass" \
  --field expect-exit-code=0 \
  --provides-evidence-for TC-B-1 >/dev/null 2>&1

before_results=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)

# First dispatch
code_change_iri="urn:dec:codechange/tc-164-test-001"
out1=$("$DEC" _dispatch code-change-committed FT-TC164-B "$code_change_iri" 2>&1)
if ! printf '%s' "$out1" | grep -q 'verdict=approved'; then
  echo "TC-164 FAIL Scenario B: first dispatch should report verdict=approved" >&2
  echo "$out1" >&2
  exit 1
fi

after_first=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)
first_delta=$((after_first - before_results))
if [ "$first_delta" -ne 1 ]; then
  echo "TC-164 FAIL Scenario B: first dispatch should create 1 result, got $first_delta" >&2
  exit 1
fi

# Second dispatch within TTL
sleep 1
out2=$("$DEC" _dispatch code-change-committed FT-TC164-B "$code_change_iri" 2>&1)
if ! printf '%s' "$out2" | grep -q 'skipped_dedup'; then
  echo "TC-164 FAIL Scenario B: second dispatch within TTL should report skipped_dedup" >&2
  echo "$out2" >&2
  exit 1
fi

after_second=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)
second_delta=$((after_second - after_first))
if [ "$second_delta" -ne 0 ]; then
  echo "TC-164 FAIL Scenario B: second dispatch within TTL should be skipped, but created $second_delta results" >&2
  exit 1
fi

# Wait for TTL to expire
sleep 6

# Third dispatch after TTL
out3=$("$DEC" _dispatch code-change-committed FT-TC164-B "$code_change_iri" 2>&1)
if ! printf '%s' "$out3" | grep -q 'verdict=approved'; then
  echo "TC-164 FAIL Scenario B: third dispatch after TTL should report verdict=approved" >&2
  echo "$out3" >&2
  exit 1
fi

after_third=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)
third_delta=$((after_third - after_second))
if [ "$third_delta" -ne 1 ]; then
  echo "TC-164 FAIL Scenario B: third dispatch after TTL should create 1 result, got $third_delta" >&2
  exit 1
fi

echo "TC-164: Scenario B PASS"

# --- Scenario C: dedup is per-(key,env), not global -------------------------
echo "TC-164: Scenario C — per-key dedup"

# Author first graph and dispatch
"$DEC" verify graph new --id VG-302 --verifies FT-TC164-A \
  --bench BNCH-001-ephemeral-cli >/dev/null 2>&1

"$DEC" verify step add VG-302 --type shell-command \
  --field command="echo pass" \
  --field expect-exit-code=0 >/dev/null 2>&1

before=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)

out1=$("$DEC" _dispatch graph-accepted VG-302 2>&1)
if ! printf '%s' "$out1" | grep -q 'dispatched=1'; then
  echo "TC-164 FAIL Scenario C: first dispatch should succeed" >&2
  exit 1
fi

after_first=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)

# Immediately author and dispatch second graph (different key)
"$DEC" verify graph new --id VG-303 --verifies FT-TC164-A \
  --bench BNCH-001-ephemeral-cli >/dev/null 2>&1

"$DEC" verify step add VG-303 --type shell-command \
  --field command="echo pass" \
  --field expect-exit-code=0 >/dev/null 2>&1

out2=$("$DEC" _dispatch graph-accepted VG-303 2>&1)
if ! printf '%s' "$out2" | grep -q 'dispatched=1'; then
  echo "TC-164 FAIL Scenario C: second dispatch (different key) should succeed" >&2
  echo "$out2" >&2
  exit 1
fi

after_second=$(find .dec/verify/result -name "*.ttl" 2>/dev/null | wc -l)

# Both dispatches should succeed (dedup is per-key)
total_delta=$((after_second - before))
if [ "$total_delta" -ne 2 ]; then
  echo "TC-164 FAIL Scenario C: expected 2 results for 2 different keys, got $total_delta" >&2
  exit 1
fi

echo "TC-164: Scenario C PASS"

echo "TC-164 PASS"
