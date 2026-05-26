#!/usr/bin/env bash
# TC-160 — dec verify feature exits 3 when any TC has no covering graph
# even if covered TCs all pass (FT-099 / ADR-031).
#
# Three scenarios:
#   A — covered TCs pass, one uncovered → exit 3, coverage gap listed
#   B — all TCs uncovered (no graphs at all) → exit 3
#   Cross-check — TC-159's fully-covered fixture exits 0
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

W="$(mktemp -d --tmpdir tc-160.XXXXXX)"
trap 'rm -rf "$W"' EXIT
cd "$W"

# Scenario A — covered TCs pass, one uncovered.
"$DEC" init --template engineering-development >/dev/null 2>&1
mkdir -p .product/features
cat >.product/features/FT-GAP.md <<'EOF'
---
id: FT-GAP
title: TC-160 fixture A
tests: [TC-COV-A, TC-COV-B, TC-COV-C]
---
fixture body
EOF
"$DEC" verify graph new --id VG-001-cov-ab --verifies FT-GAP \
  --environment ENV-001-ephemeral-cli >/dev/null
"$DEC" verify step add VG-001-cov-ab --type shell-command \
  --field command="echo ok" \
  --field expect-exit-code=0 \
  --provides-evidence-for TC-COV-A \
  --provides-evidence-for TC-COV-B >/dev/null

rc=0
OUT_A="$("$DEC" verify feature FT-GAP 2>&1)" || rc=$?
if [ "$rc" -ne 3 ]; then
  echo "TC-160 FAIL A: expected exit 3 (coverage gap); got $rc" >&2
  printf '%s\n' "$OUT_A" >&2
  exit 1
fi
for needle in "VG-001-cov-ab (ENV-001-ephemeral-cli) → approved" \
              "TC-COV-A" "TC-COV-B" "TC-COV-C" \
              "Coverage gaps:" "TC-COV-C" \
              "dec verify graph generate FT-GAP"; do
  if ! printf '%s' "$OUT_A" | grep -q "$needle"; then
    echo "TC-160 FAIL A: missing needle '$needle'" >&2
    printf '%s\n' "$OUT_A" >&2
    exit 1
  fi
done
if ! printf '%s' "$OUT_A" | grep -E 'TC-COV-A\s+approved' >/dev/null; then
  echo "TC-160 FAIL A: TC-COV-A not approved in per-TC table" >&2
  printf '%s\n' "$OUT_A" >&2
  exit 1
fi
if ! printf '%s' "$OUT_A" | grep -E 'TC-COV-B\s+approved' >/dev/null; then
  echo "TC-160 FAIL A: TC-COV-B not approved in per-TC table" >&2
  printf '%s\n' "$OUT_A" >&2
  exit 1
fi
if ! printf '%s' "$OUT_A" | grep -E 'TC-COV-C\s+rejected' >/dev/null; then
  echo "TC-160 FAIL A: TC-COV-C not rejected in per-TC table" >&2
  printf '%s\n' "$OUT_A" >&2
  exit 1
fi

# Scenario B — feature with TCs but no graphs at all.
cat >.product/features/FT-EMPTY.md <<'EOF'
---
id: FT-EMPTY
title: TC-160 fixture B
tests: [TC-NO-A, TC-NO-B]
---
fixture body
EOF
rc=0
OUT_B="$("$DEC" verify feature FT-EMPTY 2>&1)" || rc=$?
if [ "$rc" -ne 3 ]; then
  echo "TC-160 FAIL B: expected exit 3 (no graphs at all); got $rc" >&2
  printf '%s\n' "$OUT_B" >&2
  exit 1
fi
if ! printf '%s' "$OUT_B" | grep -q 'Coverage gaps:.*TC-NO-A'; then
  echo "TC-160 FAIL B: TC-NO-A not in coverage gaps" >&2
  printf '%s\n' "$OUT_B" >&2
  exit 1
fi
if ! printf '%s' "$OUT_B" | grep -q 'Coverage gaps:.*TC-NO-B'; then
  echo "TC-160 FAIL B: TC-NO-B not in coverage gaps" >&2
  printf '%s\n' "$OUT_B" >&2
  exit 1
fi
if ! printf '%s' "$OUT_B" | grep -q 'Aggregate verdict: rejected'; then
  echo "TC-160 FAIL B: missing Aggregate verdict: rejected" >&2
  printf '%s\n' "$OUT_B" >&2
  exit 1
fi

# Cross-check — fully covered fixture exits 0.
cat >.product/features/FT-OK.md <<'EOF'
---
id: FT-OK
title: TC-160 cross-check
tests: [TC-OK-A]
---
fixture body
EOF
"$DEC" verify graph new --id VG-002-ok --verifies FT-OK \
  --environment ENV-001-ephemeral-cli >/dev/null
"$DEC" verify step add VG-002-ok --type shell-command \
  --field command="echo ok" \
  --field expect-exit-code=0 \
  --provides-evidence-for TC-OK-A >/dev/null
rc=0
OUT_C="$("$DEC" verify feature FT-OK 2>&1)" || rc=$?
if [ "$rc" -ne 0 ]; then
  echo "TC-160 FAIL cross-check: fully-covered feature expected exit 0; got $rc" >&2
  printf '%s\n' "$OUT_C" >&2
  exit 1
fi
if ! printf '%s' "$OUT_C" | grep -q 'Coverage gaps: none'; then
  echo "TC-160 FAIL cross-check: missing 'Coverage gaps: none'" >&2
  printf '%s\n' "$OUT_C" >&2
  exit 1
fi

echo "TC-160 PASS"
