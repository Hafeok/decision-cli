#!/usr/bin/env bash
# TC-164 — subscription dedup TTL coalesces rapid re-fires within the
# window into a single dispatch (FT-100).
#
# Four scenarios exercise the dedup ledger:
#   A — graph_accepted dedup window: replay within TTL is coalesced.
#   B — code_change_committed dedup window: same idea on the other ledger.
#   C — per-(key, env) keying: different keys dispatch independently.
#   D — ledger persistence across "restart": stop/start the CLI process.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-164.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"
"$DEC" init --template engineering-development >/dev/null 2>&1

# Use a 5 s TTL so we can wait past it without the test taking forever.
TTL=5
cat >>.dec/config.toml <<EOF

[verify_graph_runner.on_graph_accepted]
enabled = true
dedup_ttl_seconds = $TTL

[verify_graph_runner.on_code_change]
enabled = true
dedup_ttl_seconds = $TTL
EOF

# Seed a fixture feature with one graph + one TC.
mkdir -p .product/features
cat >.product/features/FT-DEDUP.md <<'EOF'
---
id: FT-DEDUP
title: TC-164 fixture
tests: [TC-DED]
---
fixture
EOF
"$DEC" verify graph new --id VG-301 --verifies FT-DEDUP \
  --environment ENV-001-ephemeral-cli >/dev/null
"$DEC" verify step add VG-301 --type shell-command \
  --field command="echo ok" \
  --field expect-exit-code=0 \
  --provides-evidence-for TC-DED >/dev/null

# The step add above already fired graph_accepted_dispatch for VG-301.
# Count sessions tagged with VG-301.
COUNT_BEFORE_A=$("$DEC" session list --limit 200 2>&1 | grep -c 'verify-graph-runner/VG-301/' || true)
if [ "$COUNT_BEFORE_A" -lt 1 ]; then
  echo "TC-164 FAIL A: VG-301 did not fire on step_add" >&2
  exit 1
fi

# ----- Scenario A: graph_accepted dedup within TTL ----------------------
REPLAY_A="$("$DEC" _dispatch graph-accepted VG-301 2>&1)"
if ! printf '%s' "$REPLAY_A" | grep -q 'skipped_dedup=1'; then
  echo "TC-164 FAIL A: within-TTL replay was not coalesced" >&2
  printf '%s\n' "$REPLAY_A" >&2
  exit 1
fi
COUNT_AFTER_REPLAY_A=$("$DEC" session list --limit 200 2>&1 | grep -c 'verify-graph-runner/VG-301/' || true)
if [ "$COUNT_AFTER_REPLAY_A" -ne "$COUNT_BEFORE_A" ]; then
  echo "TC-164 FAIL A: dedup let an extra session through (before=$COUNT_BEFORE_A after=$COUNT_AFTER_REPLAY_A)" >&2
  exit 1
fi

# Wait past the TTL with a safety margin.
sleep $((TTL + 2))

# After the TTL the replay must dispatch again.
REFIRE_A="$("$DEC" _dispatch graph-accepted VG-301 2>&1)"
if ! printf '%s' "$REFIRE_A" | grep -q 'dispatched=1'; then
  echo "TC-164 FAIL A: post-TTL replay was not re-dispatched" >&2
  printf '%s\n' "$REFIRE_A" >&2
  exit 1
fi
COUNT_FINAL_A=$("$DEC" session list --limit 200 2>&1 | grep -c 'verify-graph-runner/VG-301/' || true)
if [ "$COUNT_FINAL_A" -le "$COUNT_BEFORE_A" ]; then
  echo "TC-164 FAIL A: expected new session post-TTL (before=$COUNT_BEFORE_A final=$COUNT_FINAL_A)" >&2
  exit 1
fi

# ----- Scenario B: code_change_committed dedup window -------------------
CC_IRI="urn:dec:codechange/dedup-1"

OUT_B1="$("$DEC" _dispatch code-change-committed FT-DEDUP "$CC_IRI" 2>&1)"
if printf '%s' "$OUT_B1" | grep -q 'skipped_dedup'; then
  echo "TC-164 FAIL B: first code-change dispatch was unexpectedly dedup'd" >&2
  printf '%s\n' "$OUT_B1" >&2
  exit 1
fi

# Replay within TTL — should be dedup'd.
OUT_B2="$("$DEC" _dispatch code-change-committed FT-DEDUP "$CC_IRI" 2>&1)"
if ! printf '%s' "$OUT_B2" | grep -q 'skipped_dedup'; then
  echo "TC-164 FAIL B: within-TTL code-change replay was not coalesced" >&2
  printf '%s\n' "$OUT_B2" >&2
  exit 1
fi

# Wait past TTL, refire — should dispatch.
sleep $((TTL + 2))
OUT_B3="$("$DEC" _dispatch code-change-committed FT-DEDUP "$CC_IRI" 2>&1)"
if printf '%s' "$OUT_B3" | grep -q 'skipped_dedup'; then
  echo "TC-164 FAIL B: post-TTL code-change replay was wrongly dedup'd" >&2
  printf '%s\n' "$OUT_B3" >&2
  exit 1
fi
if ! printf '%s' "$OUT_B3" | grep -qE 'verdict='; then
  echo "TC-164 FAIL B: post-TTL refire did not produce a verdict" >&2
  printf '%s\n' "$OUT_B3" >&2
  exit 1
fi

# ----- Scenario C: per-(key, env) keying -------------------------------
# Different code-change IRI must dispatch immediately even within the
# TTL of the previous one.
DIFFERENT_CC="urn:dec:codechange/dedup-different"
OUT_C="$("$DEC" _dispatch code-change-committed FT-DEDUP "$DIFFERENT_CC" 2>&1)"
if printf '%s' "$OUT_C" | grep -q 'skipped_dedup'; then
  echo "TC-164 FAIL C: different code-change IRI was wrongly dedup'd" >&2
  printf '%s\n' "$OUT_C" >&2
  exit 1
fi

# Different graph key must dispatch immediately.
"$DEC" verify graph new --id VG-302 --verifies FT-DEDUP \
  --environment ENV-001-ephemeral-cli >/dev/null
# Graph_new alone does not fire (no steps). Add a step → fires.
"$DEC" verify step add VG-302 --type shell-command \
  --field command="echo ok" \
  --field expect-exit-code=0 \
  --provides-evidence-for TC-DED >/dev/null
COUNT_C=$("$DEC" session list --limit 200 2>&1 | grep -c 'verify-graph-runner/VG-302/' || true)
if [ "$COUNT_C" -lt 1 ]; then
  echo "TC-164 FAIL C: VG-302 was wrongly dedup'd against VG-301's ledger" >&2
  exit 1
fi

# ----- Scenario D: ledger persistence across "restart" ------------------
# Trigger graph-accepted for VG-302 (which is still in its TTL window).
# Then the next invocation in a fresh process MUST honour the ledger.
REPLAY_D1="$("$DEC" _dispatch graph-accepted VG-302 2>&1)"
if ! printf '%s' "$REPLAY_D1" | grep -q 'skipped_dedup=1'; then
  echo "TC-164 FAIL D: in-process replay did not coalesce" >&2
  printf '%s\n' "$REPLAY_D1" >&2
  exit 1
fi

# Each `dec` invocation is a fresh process — the previous call already
# proved cross-process behaviour. Run another fresh process and assert
# again to be explicit.
REPLAY_D2="$("$DEC" _dispatch graph-accepted VG-302 2>&1)"
if ! printf '%s' "$REPLAY_D2" | grep -q 'skipped_dedup=1'; then
  echo "TC-164 FAIL D: ledger did not persist across processes" >&2
  printf '%s\n' "$REPLAY_D2" >&2
  exit 1
fi

echo "TC-164 PASS"
