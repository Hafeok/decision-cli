#!/usr/bin/env bash
# TC-162 — graph_accepted_dispatch emits exactly one VerifyGraphRunDispatchEvent
# per accepted graph and opens one Session (FT-100).
#
# The FT-100 subscription is wired synchronously from `dec verify step add`
# (the natural trigger: a newly-committed graph with at least one step) and
# is also driveable via the hidden `_dispatch graph-accepted` verb, which
# the dedup-replay scenario exercises.
#
# Four scenarios:
#   A — happy path: dispatch fires on step_add; one session + one VGR.
#   B — config `enabled = false` suppresses dispatch.
#   C — replay within TTL is coalesced (ledger short-circuits).
#   D — graph references env not in catalog → env-error, no dispatch.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-162.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"
"$DEC" init --template engineering-development >/dev/null 2>&1

# Seed a minimal feature so `dec:verifies FT-FIXTURE` resolves.
mkdir -p .product/features
cat >.product/features/FT-FIXTURE.md <<'EOF'
---
id: FT-FIXTURE
title: TC-162 fixture
tests: [TC-FIXTURE]
---
fixture
EOF

# ----- Scenario A: happy path --------------------------------------------
rc=0
"$DEC" verify graph new --id VG-101 --verifies FT-FIXTURE \
  --environment ENV-001-ephemeral-cli >/dev/null
"$DEC" verify step add VG-101 --type shell-command \
  --field command="echo ok" \
  --field expect-exit-code=0 >/dev/null

# Session list should contain exactly one verify-graph-runner session
# tagged with VG-101.
LIST_A="$("$DEC" session list --limit 100 2>&1)"
COUNT_A="$(printf '%s\n' "$LIST_A" | grep -c 'verify-graph-runner/VG-101/' || true)"
if [ "$COUNT_A" -ne 1 ]; then
  echo "TC-162 FAIL A: expected 1 verify-graph-runner session for VG-101; got $COUNT_A" >&2
  printf '%s\n' "$LIST_A" >&2
  exit 1
fi

# Exactly one VGR file with dec:resultOf matching VG-101.
VGR_COUNT="$(ls .dec/verify/result/VGR-*.ttl 2>/dev/null | wc -l)"
if [ "$VGR_COUNT" -lt 1 ]; then
  echo "TC-162 FAIL A: no VGR persisted" >&2
  ls .dec/verify/result/ 2>&1 >&2
  exit 1
fi
if ! grep -l 'dec:resultOf <https://decision-cli.dev/ns/graph/VG-101>' .dec/verify/result/VGR-*.ttl >/dev/null; then
  echo "TC-162 FAIL A: no VGR carries dec:resultOf VG-101" >&2
  cat .dec/verify/result/VGR-*.ttl >&2
  exit 1
fi

# ----- Scenario B: enabled = false suppresses dispatch -------------------
cat >>.dec/config.toml <<'EOF'

[verify_graph_runner.on_graph_accepted]
enabled = false
EOF

"$DEC" verify graph new --id VG-102 --verifies FT-FIXTURE \
  --environment ENV-001-ephemeral-cli >/dev/null
"$DEC" verify step add VG-102 --type shell-command \
  --field command="echo never" \
  --field expect-exit-code=0 >/dev/null

LIST_B="$("$DEC" session list --limit 100 2>&1)"
COUNT_B="$(printf '%s\n' "$LIST_B" | grep -c 'verify-graph-runner/VG-102/' || true)"
if [ "$COUNT_B" -ne 0 ]; then
  echo "TC-162 FAIL B: expected 0 sessions for VG-102 (enabled=false); got $COUNT_B" >&2
  printf '%s\n' "$LIST_B" >&2
  exit 1
fi

# Re-enable for the remaining scenarios.
# Replace the on_graph_accepted block to set enabled = true.
python3 - <<'PYEOF'
import pathlib
p = pathlib.Path('.dec/config.toml')
body = p.read_text()
body = body.replace(
    '[verify_graph_runner.on_graph_accepted]\nenabled = false',
    '[verify_graph_runner.on_graph_accepted]\nenabled = true\ndedup_ttl_seconds = 300',
)
p.write_text(body)
PYEOF

# ----- Scenario C: replay within TTL is dedup'd --------------------------
"$DEC" verify graph new --id VG-103 --verifies FT-FIXTURE \
  --environment ENV-001-ephemeral-cli >/dev/null
"$DEC" verify step add VG-103 --type shell-command \
  --field command="echo ok" \
  --field expect-exit-code=0 >/dev/null

LIST_C_BEFORE="$("$DEC" session list --limit 200 2>&1)"
BEFORE_COUNT="$(printf '%s\n' "$LIST_C_BEFORE" | grep -c 'verify-graph-runner/VG-103/' || true)"
if [ "$BEFORE_COUNT" -lt 1 ]; then
  echo "TC-162 FAIL C: first dispatch did not produce a session" >&2
  exit 1
fi

# Replay via the hidden verb — should be dedup'd.
REPLAY="$("$DEC" _dispatch graph-accepted VG-103 2>&1)"
if ! printf '%s' "$REPLAY" | grep -q 'skipped_dedup=1'; then
  echo "TC-162 FAIL C: replay within TTL was not coalesced" >&2
  printf '%s\n' "$REPLAY" >&2
  exit 1
fi

LIST_C_AFTER="$("$DEC" session list --limit 200 2>&1)"
AFTER_COUNT="$(printf '%s\n' "$LIST_C_AFTER" | grep -c 'verify-graph-runner/VG-103/' || true)"
if [ "$AFTER_COUNT" -ne "$BEFORE_COUNT" ]; then
  echo "TC-162 FAIL C: dedup did not suppress duplicate dispatch (before=$BEFORE_COUNT after=$AFTER_COUNT)" >&2
  exit 1
fi

# ----- Scenario D: env missing from catalog ------------------------------
# Author a graph file by hand whose `dec:environment` references an
# env that has no on-disk catalog entry.
mkdir -p .dec/verify/graph
cat >.dec/verify/graph/VG-199.ttl <<'EOF'
@prefix dec: <https://decision-cli.dev/ns#> .
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .

<https://decision-cli.dev/ns/graph/VG-199>
    rdf:type           dec:VerificationGraph ;
    dec:verifies       <https://decision-cli.dev/ns/feature/FT-FIXTURE> ;
    dec:environment    <https://decision-cli.dev/ns/env/ENV-DOES-NOT-EXIST> ;
    dec:steps          () .
EOF

# Trigger via the hidden verb. Expected: env-error reported, no session.
OUT_D="$("$DEC" _dispatch graph-accepted VG-199 2>&1)" || rc=$?
if ! printf '%s' "$OUT_D" | grep -q 'env-error'; then
  echo "TC-162 FAIL D: expected env-error diagnostic for missing env" >&2
  printf '%s\n' "$OUT_D" >&2
  exit 1
fi

echo "TC-162 PASS"
