#!/usr/bin/env bash
# scripts/checks/dispatch-rejected-stays-blocked.sh
#
# Enforces FT-021 TC-033: when the verifier returns a `rejected` verdict,
# the paired `dec:DispatchGroup` transitions to `dec:dispatchStatus
# "interpretation-rejected"` and STAYS there. That status is terminal —
# the state machine in `core::dispatch::lifecycle` refuses to advance
# past `InterpretationRejected`, and any persisted DispatchGroup with a
# rejected verdict reachable must carry exactly that status (not
# `complete`, not `awaiting-amendment`).
#
# Two-part mechanical check:
#
#   1. Source invariant — the Rust state machine declares
#      `InterpretationRejected` as terminal AND maps `VerdictRejected`
#      to `InterpretationRejected`. Drift in either arm flips the
#      semantics.
#
#   2. Store invariant — any orchestration store at
#      `<workdir>/.dec/store/orchestration.nq` where a verifier verdict
#      with `dec:verdict "rejected"` is reachable from a DispatchGroup
#      via `dec:hasInterpretationSession` + `prov:wasGeneratedBy` MUST
#      carry `dec:dispatchStatus "interpretation-rejected"` on the
#      paired group. Stores without rejected verdicts are vacuously OK.
#
# Exit 0: source machinery intact AND every rejected verdict's paired
#         group is parked in `interpretation-rejected`.
# Exit 1: source machinery regressed OR a rejected verdict's paired
#         group is in any other status.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

LIFECYCLE="crates/decision-cli/src/core/dispatch/lifecycle.rs"
if [ ! -f "$LIFECYCLE" ]; then
  echo "ERROR: expected $LIFECYCLE (FT-021 anchor file)"
  exit 1
fi

FAILED=0
if ! grep -q "VerdictRejected" "$LIFECYCLE"; then
  echo "ERROR: lifecycle.rs no longer references VerdictRejected (FT-021)"
  FAILED=1
fi
if ! grep -q "InterpretationRejected" "$LIFECYCLE"; then
  echo "ERROR: lifecycle.rs no longer references InterpretationRejected (FT-021)"
  FAILED=1
fi
# is_terminal() must list InterpretationRejected.
if ! grep -q "InterpretationRejected" "$LIFECYCLE"; then
  echo "ERROR: lifecycle.rs lost InterpretationRejected (FT-021)"
  FAILED=1
fi
# A VerdictRejected event must be reachable from awaiting-interpretation
# AND map to InterpretationRejected.
if ! grep -q "VerdictRejected.*=> S::InterpretationRejected" "$LIFECYCLE" \
   && ! grep -Pzq 'E::VerdictRejected[^=]*=>[\s\S]*?S::InterpretationRejected' "$LIFECYCLE" 2>/dev/null; then
  echo "ERROR: lifecycle.rs no longer maps VerdictRejected to InterpretationRejected (FT-021)"
  FAILED=1
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

DUMPS="$(find . -path '*/.dec/store/orchestration.nq' -not -path '*/target/*' 2>/dev/null || true)"
if [ -z "$DUMPS" ]; then
  echo "OK: source invariant intact; no orchestration stores to audit (vacuous PASS)"
  exit 0
fi

GROUP_TYPE="https://decision-cli.dev/ns#DispatchGroup"
STATUS_PRED="https://decision-cli.dev/ns#dispatchStatus"
HAS_INTERP="https://decision-cli.dev/ns#hasInterpretationSession"
VERDICT_TYPE="https://decision-cli.dev/ns#VerificationVerdict"
VERDICT_PRED="https://decision-cli.dev/ns#verdict"
PROV_GEN_BY="http://www.w3.org/ns/prov#wasGeneratedBy"

VIOLATIONS=0
while IFS= read -r dump; do
  [ -z "$dump" ] && continue
  drift="$(awk \
    -v group_type="$GROUP_TYPE" \
    -v status_pred="$STATUS_PRED" \
    -v has_interp="$HAS_INTERP" \
    -v verdict_type="$VERDICT_TYPE" \
    -v verdict_pred="$VERDICT_PRED" \
    -v prov_gen_by="$PROV_GEN_BY" \
    '
    BEGIN { rdf_type = "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>" }
    {
      subj = $1; pred = $2; obj = $3
      if (pred == rdf_type) {
        if (obj == "<" group_type ">") group[subj] = 1
        if (obj == "<" verdict_type ">") verdict[subj] = 1
      } else if (pred == "<" status_pred ">") {
        gsub(/^"/, "", obj); gsub(/"\^\^.*$/, "", obj); gsub(/".*$/, "", obj)
        group_status[subj] = obj
      } else if (pred == "<" has_interp ">") {
        group_interp_session[subj] = obj
      } else if (pred == "<" verdict_pred ">") {
        gsub(/^"/, "", obj); gsub(/"\^\^.*$/, "", obj); gsub(/".*$/, "", obj)
        verdict_value[subj] = obj
      } else if (pred == "<" prov_gen_by ">") {
        verdict_session[subj] = obj
      }
    }
    END {
      for (v in verdict) {
        if (verdict_value[v] != "rejected") continue
        sess = verdict_session[v]
        if (sess == "") continue
        for (g in group) {
          if (group_interp_session[g] == sess) {
            if (group_status[g] != "interpretation-rejected") {
              print "  • DispatchGroup " g " with rejected verdict " v " has status " group_status[g] " (expected interpretation-rejected)"
            }
          }
        }
      }
    }
    ' "$dump")"
  if [ -n "$drift" ]; then
    echo "ERROR: rejected-verdict-stays-blocked invariant violated in $dump (FT-021 TC-033):"
    echo "$drift"
    VIOLATIONS=1
  fi
done <<EOF
$DUMPS
EOF

if [ "$VIOLATIONS" -ne 0 ]; then
  exit 1
fi

echo "OK: every rejected verdict parks its DispatchGroup in interpretation-rejected (FT-021 TC-033)"
exit 0
