#!/usr/bin/env bash
# TC-158 — dec verify graph run prints per-step trace, final verdict, and
# maps verdict to exit code (FT-099 / ADR-028).
#
# Five scenarios:
#   A — approved: 0
#   B — rejected with feedback: 1
#   C — amendment-required (unrunnable): 2
#   D — missing graph: 1 + ArtifactNotFound on stderr
#   E — --format json: single JSON document with the required keys
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-158.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"
"$DEC" init --template engineering-development >/dev/null 2>&1

# Seed a minimal feature so `dec:verifies FT-001` resolves.
mkdir -p .product/features
cat >.product/features/FT-001.md <<'EOF'
---
id: FT-001
title: TC-158 fixture
tests: [TC-EVI]
---
fixture
EOF

# VG-PASS — one passing shell-command.
"$DEC" verify graph new --id VG-001 --verifies FT-001 \
  --environment ENV-001-ephemeral-cli >/dev/null
"$DEC" verify step add VG-001 --type shell-command \
  --field command="echo ok" \
  --field expect-exit-code=0 >/dev/null

# VG-FAIL — one failing shell-command with evidence linkage.
"$DEC" verify graph new --id VG-002 --verifies FT-001 \
  --environment ENV-001-ephemeral-cli >/dev/null
"$DEC" verify step add VG-002 --type shell-command \
  --field command="exit 1" \
  --field expect-exit-code=0 \
  --provides-evidence-for TC-EVI >/dev/null

# VG-AMEND — sparql-assertion over a missing target → unrunnable →
# amendment-required (no provides-evidence-for so the empty-evidence
# branch of FT-097 single_graph_verdict applies). With no evidence
# linkage on a failure step, the verdict is amendment-required.
"$DEC" verify graph new --id VG-003 --verifies FT-001 \
  --environment ENV-001-ephemeral-cli >/dev/null
"$DEC" verify step add VG-003 --type sparql-assertion \
  --field target="nonexistent.ttl" \
  --field query="SELECT * WHERE { ?s ?p ?o }" >/dev/null

# Scenario A — approved.
rc=0
OUT_PASS="$("$DEC" verify graph run VG-001 2>&1)" || rc=$?
if [ "$rc" -ne 0 ]; then
  echo "TC-158 FAIL A: VG-001 expected exit 0; got $rc" >&2
  printf '%s\n' "$OUT_PASS" >&2
  exit 1
fi
if ! printf '%s' "$OUT_PASS" | grep -Eq '^\s*\[0\] shell-command\s+pass'; then
  echo "TC-158 FAIL A: missing pass row" >&2
  printf '%s\n' "$OUT_PASS" >&2
  exit 1
fi
if ! printf '%s' "$OUT_PASS" | grep -q 'Verdict: approved'; then
  echo "TC-158 FAIL A: missing Verdict: approved" >&2
  printf '%s\n' "$OUT_PASS" >&2
  exit 1
fi
if ! printf '%s' "$OUT_PASS" | grep -q 'Result:.*VGR-'; then
  echo "TC-158 FAIL A: missing Result: VGR row" >&2
  printf '%s\n' "$OUT_PASS" >&2
  exit 1
fi
if printf '%s' "$OUT_PASS" | grep -q '^Feedback:'; then
  echo "TC-158 FAIL A: unexpected Feedback line in approved run" >&2
  printf '%s\n' "$OUT_PASS" >&2
  exit 1
fi

# Scenario B — rejected with feedback.
rc=0
OUT_FAIL="$("$DEC" verify graph run VG-002 2>&1)" || rc=$?
if [ "$rc" -ne 1 ]; then
  echo "TC-158 FAIL B: VG-002 expected exit 1; got $rc" >&2
  printf '%s\n' "$OUT_FAIL" >&2
  exit 1
fi
if ! printf '%s' "$OUT_FAIL" | grep -q '\[0\] shell-command'; then
  echo "TC-158 FAIL B: missing shell-command row" >&2
  printf '%s\n' "$OUT_FAIL" >&2
  exit 1
fi
if ! printf '%s' "$OUT_FAIL" | grep -q 'fail'; then
  echo "TC-158 FAIL B: missing fail outcome" >&2
  printf '%s\n' "$OUT_FAIL" >&2
  exit 1
fi
if ! printf '%s' "$OUT_FAIL" | grep -q 'Verdict: rejected'; then
  echo "TC-158 FAIL B: missing Verdict: rejected" >&2
  printf '%s\n' "$OUT_FAIL" >&2
  exit 1
fi
# Feedback emission for evidence-bearing failure.
if ! printf '%s' "$OUT_FAIL" | grep -q '^Feedback:'; then
  echo "TC-158 FAIL B: missing Feedback: block for evidence-bearing fail" >&2
  printf '%s\n' "$OUT_FAIL" >&2
  exit 1
fi
if [ ! -d .dec/verify/result ] || ! ls .dec/verify/result/VGR-*.ttl >/dev/null 2>&1; then
  echo "TC-158 FAIL B: no VGR persisted on rejection" >&2
  exit 1
fi
if ! grep -l 'dec:verdict "rejected"' .dec/verify/result/VGR-*.ttl >/dev/null; then
  echo "TC-158 FAIL B: no VGR carries dec:verdict rejected" >&2
  exit 1
fi

# Scenario C — amendment-required (sparql over missing target).
rc=0
OUT_AMEND="$("$DEC" verify graph run VG-003 2>&1)" || rc=$?
if [ "$rc" -ne 2 ]; then
  echo "TC-158 FAIL C: VG-003 expected exit 2; got $rc" >&2
  printf '%s\n' "$OUT_AMEND" >&2
  exit 1
fi
if ! printf '%s' "$OUT_AMEND" | grep -q 'unrunnable'; then
  echo "TC-158 FAIL C: missing unrunnable outcome" >&2
  printf '%s\n' "$OUT_AMEND" >&2
  exit 1
fi
if ! printf '%s' "$OUT_AMEND" | grep -q 'Verdict: amendment-required'; then
  echo "TC-158 FAIL C: missing Verdict: amendment-required" >&2
  printf '%s\n' "$OUT_AMEND" >&2
  exit 1
fi

# Scenario D — missing graph.
rc=0
OUT_MISS="$("$DEC" verify graph run VG-999 2>&1)" || rc=$?
if [ "$rc" -ne 1 ]; then
  echo "TC-158 FAIL D: missing graph expected exit 1; got $rc" >&2
  printf '%s\n' "$OUT_MISS" >&2
  exit 1
fi
if ! printf '%s' "$OUT_MISS" | grep -qi 'not found\|VG-999'; then
  echo "TC-158 FAIL D: missing 'not found' or 'VG-999' diagnostic" >&2
  printf '%s\n' "$OUT_MISS" >&2
  exit 1
fi

# Scenario E — --format json on VG-PASS.
rc=0
OUT_JSON="$("$DEC" verify graph run VG-001 --format json)" || rc=$?
if [ "$rc" -ne 0 ]; then
  echo "TC-158 FAIL E: json approved expected exit 0; got $rc" >&2
  printf '%s\n' "$OUT_JSON" >&2
  exit 1
fi
python3 - <<PYEOF "$OUT_JSON"
import json, sys
doc = json.loads(sys.argv[1])
for key in ("session_id", "result_id", "verdict", "step_outcomes", "emitted_feedback"):
    assert key in doc, f"missing key: {key}"
assert doc["verdict"] == "approved", f"verdict not approved: {doc['verdict']}"
assert isinstance(doc["step_outcomes"], list) and len(doc["step_outcomes"]) == 1
PYEOF

echo "TC-158 PASS"
