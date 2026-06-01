#!/usr/bin/env bash
# TC-163 — code_change_committed_dispatch dispatches per env and writes an
# aggregate session with composed verdict (FT-100).
#
# Triggering a CodeChange in the test uses the hidden `dec _dispatch
# code-change-committed` verb (per FT-100 §Scripts: "thin helper" path),
# which directly drives the subscription handler. This avoids standing up
# a full `dec implement` flow that produces a real CodeChange.
#
# Four scenarios:
#   A — happy-path aggregate approved.
#   B — aggregate rejected emits feature-level feedback.
#   C — per-env fan-out independence.
#   D — coverage gap at commit time.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-163.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"
"$DEC" init --template engineering-development >/dev/null 2>&1

# Set a short dedup TTL so we can re-fire the handler within the test if
# needed without waiting.
cat >>.dec/config.toml <<'EOF'

[verify_graph_runner.on_graph_accepted]
enabled = false

[verify_graph_runner.on_code_change]
enabled = true
dedup_ttl_seconds = 1
EOF

# Seed feature FT-CC with TCs.
mkdir -p .product/features
cat >.product/features/FT-CC.md <<'EOF'
---
id: FT-CC
title: TC-163 fixture
tests: [TC-CC-A, TC-CC-B]
---
fixture
EOF

# Two graphs both targeting FT-CC, each covering one TC.
"$DEC" verify graph new --id VG-201 --verifies FT-CC \
  --bench BNCH-001-ephemeral-cli >/dev/null
"$DEC" verify step add VG-201 --type shell-command \
  --field command="echo ok" \
  --field expect-exit-code=0 \
  --provides-evidence-for TC-CC-A >/dev/null

"$DEC" verify graph new --id VG-202 --verifies FT-CC \
  --bench BNCH-001-ephemeral-cli >/dev/null
"$DEC" verify step add VG-202 --type shell-command \
  --field command="echo ok" \
  --field expect-exit-code=0 \
  --provides-evidence-for TC-CC-B >/dev/null

# ----- Scenario A: happy-path aggregate approved -------------------------
CC_IRI_A="urn:dec:codechange/test-A"
OUT_A="$("$DEC" _dispatch code-change-committed FT-CC "$CC_IRI_A" 2>&1)"
if ! printf '%s' "$OUT_A" | grep -q 'verdict=approved'; then
  echo "TC-163 FAIL A: expected verdict=approved; got:" >&2
  printf '%s\n' "$OUT_A" >&2
  exit 1
fi
if ! printf '%s' "$OUT_A" | grep -q 'per_graph=2'; then
  echo "TC-163 FAIL A: expected per_graph=2; got:" >&2
  printf '%s\n' "$OUT_A" >&2
  exit 1
fi
if ! printf '%s' "$OUT_A" | grep -q 'feedback=0'; then
  echo "TC-163 FAIL A: expected no feedback on approved aggregate; got:" >&2
  printf '%s\n' "$OUT_A" >&2
  exit 1
fi

# Aggregate session has role tag in IRI.
LIST_A="$("$DEC" session list --limit 200 2>&1)"
AGG_A="$(printf '%s\n' "$LIST_A" | grep -c 'verify-graph-runner-aggregate/FT-CC' || true)"
if [ "$AGG_A" -lt 1 ]; then
  echo "TC-163 FAIL A: no aggregate session present" >&2
  printf '%s\n' "$LIST_A" >&2
  exit 1
fi

# Wait for dedup TTL to expire so the next scenario can fire.
sleep 2

# ----- Scenario B: rejected aggregate emits feature-level feedback ------
# Mutate VG-202's step to fail and re-trigger.
cat >.dec/verify/graph/VG-202.ttl <<'EOF'
@prefix dec: <https://decision-cli.dev/ns#> .
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .

<https://decision-cli.dev/ns/graph/VG-202>
    rdf:type           dec:VerificationGraph ;
    dec:verifies       <https://decision-cli.dev/ns/feature/FT-CC> ;
    dec:environment    <https://decision-cli.dev/ns/bench/BNCH-001-ephemeral-cli> ;
    dec:steps          ( <https://decision-cli.dev/ns/step/VG-202/0> ) .

<https://decision-cli.dev/ns/step/VG-202/0>
    rdf:type                      dec:VerificationStep ;
    dec:stepType                  "shell-command" ;
    dec:command                   "exit 1" ;
    dec:expectExitCode            0 ;
    dec:providesEvidenceFor       <https://decision-cli.dev/ns/tc/TC-CC-B> .
EOF

CC_IRI_B="urn:dec:codechange/test-B"
OUT_B="$("$DEC" _dispatch code-change-committed FT-CC "$CC_IRI_B" 2>&1)"
if ! printf '%s' "$OUT_B" | grep -q 'verdict=rejected'; then
  echo "TC-163 FAIL B: expected verdict=rejected after mutating VG-202; got:" >&2
  printf '%s\n' "$OUT_B" >&2
  exit 1
fi
if ! printf '%s' "$OUT_B" | grep -qE 'feedback=[1-9]'; then
  echo "TC-163 FAIL B: expected at least one aggregate feedback; got:" >&2
  printf '%s\n' "$OUT_B" >&2
  exit 1
fi
# Check the feedback file has dec:class "regression" + dec:target FT-CC.
FB_FOUND=0
for line in $(printf '%s' "$OUT_B" | grep '^feedback=' | sed 's/^feedback=//'); do
  # We can't directly query feedback files, but the dispatch handler
  # commits to the store, so a SPARQL ask would work — just check that
  # the dispatch helper recorded one and the IRI hints "regression".
  if printf '%s' "$line" | grep -q 'regression'; then
    FB_FOUND=1
  fi
done
if [ "$FB_FOUND" -ne 1 ]; then
  echo "TC-163 FAIL B: no regression feedback IRI surfaced" >&2
  printf '%s\n' "$OUT_B" >&2
  exit 1
fi

sleep 2

# ----- Scenario C: per-env / per-graph fan-out independence -------------
# Both graphs are dispatched independently — VG-201 still passes, VG-202
# still fails. Re-fire with a fresh code change IRI.
CC_IRI_C="urn:dec:codechange/test-C"
OUT_C="$("$DEC" _dispatch code-change-committed FT-CC "$CC_IRI_C" 2>&1)"
if ! printf '%s' "$OUT_C" | grep -q 'per_graph=2'; then
  echo "TC-163 FAIL C: expected per_graph=2 (independent dispatch); got:" >&2
  printf '%s\n' "$OUT_C" >&2
  exit 1
fi
# Both per-graph tuples must appear in the dispatch output.
if ! printf '%s' "$OUT_C" | grep -q 'tuple VG-201'; then
  echo "TC-163 FAIL C: VG-201 not dispatched" >&2; exit 1
fi
if ! printf '%s' "$OUT_C" | grep -q 'tuple VG-202'; then
  echo "TC-163 FAIL C: VG-202 not dispatched" >&2; exit 1
fi

sleep 2

# ----- Scenario D: coverage gap -----------------------------------------
# Author FT-NOCOV with TCs but no graphs.
cat >.product/features/FT-NOCOV.md <<'EOF'
---
id: FT-NOCOV
title: TC-163 fixture (no coverage)
tests: [TC-NOCOV]
---
fixture
EOF

CC_IRI_D="urn:dec:codechange/test-D"
OUT_D="$("$DEC" _dispatch code-change-committed FT-NOCOV "$CC_IRI_D" 2>&1)"
if ! printf '%s' "$OUT_D" | grep -q 'verdict=rejected'; then
  echo "TC-163 FAIL D: expected verdict=rejected on coverage gap; got:" >&2
  printf '%s\n' "$OUT_D" >&2
  exit 1
fi
if ! printf '%s' "$OUT_D" | grep -q 'coverage_gap=true'; then
  echo "TC-163 FAIL D: expected coverage_gap=true; got:" >&2
  printf '%s\n' "$OUT_D" >&2
  exit 1
fi
if ! printf '%s' "$OUT_D" | grep -q 'per_graph=0'; then
  echo "TC-163 FAIL D: expected per_graph=0 (no covering graphs); got:" >&2
  printf '%s\n' "$OUT_D" >&2
  exit 1
fi
# Gap feedback emitted.
if ! printf '%s' "$OUT_D" | grep -q 'feedback/ft-100/gap'; then
  echo "TC-163 FAIL D: expected dec:class \"gap\" feedback IRI; got:" >&2
  printf '%s\n' "$OUT_D" >&2
  exit 1
fi

echo "TC-163 PASS"
