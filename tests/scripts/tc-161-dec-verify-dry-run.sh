#!/usr/bin/env bash
# TC-161 — dec verify graph run --dry-run and dec verify feature
# --dry-run write no artifacts and open no sessions (FT-099 / ADR-028).
#
# Per the FT-099 surface, the slice ships --dry-run on `dec verify
# feature` only; the graph-level verb does not advertise --dry-run.
# Scenarios:
#   A — `dec verify feature FT-DRY --dry-run` writes nothing
#   B — `dec verify graph run VG-001 --dry-run` is rejected by clap
#   C — `dec verify feature FT-DRY --dry-run --format json` shape
#   D — reuse enumeration: after a real run, dry-run lists fresh tuples
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

W="$(mktemp -d --tmpdir tc-161.XXXXXX)"
trap 'rm -rf "$W"' EXIT
cd "$W"

"$DEC" init --template engineering-development >/dev/null 2>&1
mkdir -p .product/features
cat >.product/features/FT-DRY.md <<'EOF'
---
id: FT-DRY
title: TC-161 fixture
tests: [TC-DRY-A, TC-DRY-B]
---
fixture body
EOF
# Two graphs with shell-commands that would create sentinel files if run.
"$DEC" verify graph new --id VG-001 --verifies FT-DRY \
  --environment ENV-001-ephemeral-cli >/dev/null
"$DEC" verify step add VG-001 --type shell-command \
  --field command="touch $W/sentinel-vg-001.txt && echo ok" \
  --field expect-exit-code=0 \
  --provides-evidence-for TC-DRY-A >/dev/null
"$DEC" verify graph new --id VG-002 --verifies FT-DRY \
  --environment ENV-001-ephemeral-cli >/dev/null
"$DEC" verify step add VG-002 --type shell-command \
  --field command="touch $W/sentinel-vg-002.txt && echo ok" \
  --field expect-exit-code=0 \
  --provides-evidence-for TC-DRY-B >/dev/null

PRE_VGR_COUNT=0
if [ -d .dec/verify/result ]; then
  PRE_VGR_COUNT=$(ls .dec/verify/result/*.ttl 2>/dev/null | wc -l)
fi

# Scenario A — dec verify feature FT-DRY --dry-run.
rc=0
OUT_A="$("$DEC" verify feature FT-DRY --dry-run 2>&1)" || rc=$?
if [ "$rc" -ne 0 ]; then
  echo "TC-161 FAIL A: dry-run expected exit 0; got $rc" >&2
  printf '%s\n' "$OUT_A" >&2
  exit 1
fi
for needle in "Would run: VG-001 (ENV-001-ephemeral-cli)" \
              "Would run: VG-002 (ENV-001-ephemeral-cli)" \
              "Would reuse: (none)"; do
  if ! printf '%s' "$OUT_A" | grep -q "$needle"; then
    echo "TC-161 FAIL A: missing needle '$needle'" >&2
    printf '%s\n' "$OUT_A" >&2
    exit 1
  fi
done
# Verify nothing was written.
POST_VGR_COUNT=0
if [ -d .dec/verify/result ]; then
  POST_VGR_COUNT=$(ls .dec/verify/result/*.ttl 2>/dev/null | wc -l)
fi
if [ "$POST_VGR_COUNT" -ne "$PRE_VGR_COUNT" ]; then
  echo "TC-161 FAIL A: dry-run wrote VGR artifacts ($PRE_VGR_COUNT → $POST_VGR_COUNT)" >&2
  exit 1
fi
if [ -e sentinel-vg-001.txt ] || [ -e sentinel-vg-002.txt ]; then
  echo "TC-161 FAIL A: sentinel files exist — runner executed during dry-run" >&2
  exit 1
fi

# Scenario B — graph-level --dry-run is rejected by clap.
rc=0
OUT_B="$("$DEC" verify graph run VG-001 --dry-run 2>&1)" || rc=$?
if [ "$rc" -eq 0 ]; then
  echo "TC-161 FAIL B: graph-level --dry-run accepted; expected clap rejection" >&2
  printf '%s\n' "$OUT_B" >&2
  exit 1
fi
if ! printf '%s' "$OUT_B" | grep -qi 'dry-run\|unexpected\|unrecognized\|error'; then
  echo "TC-161 FAIL B: missing usage diagnostic" >&2
  printf '%s\n' "$OUT_B" >&2
  exit 1
fi

# Scenario C — JSON shape on dry-run.
rc=0
OUT_C="$("$DEC" verify feature FT-DRY --dry-run --format json)" || rc=$?
if [ "$rc" -ne 0 ]; then
  echo "TC-161 FAIL C: json dry-run expected exit 0; got $rc" >&2
  printf '%s\n' "$OUT_C" >&2
  exit 1
fi
python3 - <<PYEOF "$OUT_C"
import json, sys
d = json.loads(sys.argv[1])
assert d.get("dry_run") is True, f"dry_run not True: {d.get('dry_run')!r}"
wr = d.get("would_run") or (d.get("enumeration") or {}).get("would_run") or []
assert isinstance(wr, list) and len(wr) == 2, f"would_run unexpected: {wr!r}"
PYEOF

# Scenario D — reuse enumeration vibes: after a real run, the dry-run
# still lists tuples (v1 always re-runs; would_reuse stays empty). The
# slice ships the reuse-vs-rerun *behaviour* as "always re-run" so this
# scenario asserts only that the dry-run is repeatable.
rc=0
OUT_D_REAL="$("$DEC" verify feature FT-DRY 2>&1)" || rc=$?
if [ "$rc" -ne 0 ]; then
  echo "TC-161 FAIL D: real run expected exit 0; got $rc" >&2
  printf '%s\n' "$OUT_D_REAL" >&2
  exit 1
fi
rc=0
OUT_D_DRY="$("$DEC" verify feature FT-DRY --dry-run 2>&1)" || rc=$?
if [ "$rc" -ne 0 ]; then
  echo "TC-161 FAIL D: dry-run after real run expected exit 0; got $rc" >&2
  printf '%s\n' "$OUT_D_DRY" >&2
  exit 1
fi
if ! printf '%s' "$OUT_D_DRY" | grep -q "Would run:"; then
  echo "TC-161 FAIL D: dry-run lost enumeration" >&2
  printf '%s\n' "$OUT_D_DRY" >&2
  exit 1
fi

echo "TC-161 PASS"
