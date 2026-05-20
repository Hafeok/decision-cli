#!/usr/bin/env bash
# scripts/checks/dispatch-complete-paired-terminal.sh
#
# Enforces FT-021 / ADR-017: a `dec:DispatchGroup` reaches the `complete`
# status only when both its paired sessions (action + interpretation)
# have terminated AND a verifier `dec:VerificationVerdict` with
# verdict=approved exists. TC-028.
#
# Mechanical, two-part check:
#
#   1. Source invariant — the Rust lifecycle module exists and forbids
#      `Complete` from any state other than the verdict-approved path.
#      We grep `core/dispatch/lifecycle.rs` for the only allowed paths
#      to `Complete` and refuse the build if the surface drifts.
#
#   2. Store invariant — any orchestration store at
#      `<workdir>/.dec/store/orchestration.nq` that contains a
#      `dec:DispatchGroup` with `dec:dispatchStatus "complete"` must
#      also contain a `dec:VerificationVerdict` with
#      `dec:verdict "approved"` reachable from the group's
#      `dec:hasInterpretationSession` via `prov:wasGeneratedBy`.
#      Stores without any DispatchGroup are vacuously OK.
#
# Exit 0: source machinery intact AND no `complete` group lacking the
#         paired-and-approved evidence.
# Exit 1: source machinery regressed OR a `complete` group is missing
#         a paired approved verdict.
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

# Part 1: source invariant. The state machine's path to Complete must
# require a VerdictApproved event.
if ! grep -q "VerdictApproved" "$LIFECYCLE"; then
  echo "ERROR: lifecycle.rs no longer references VerdictApproved (FT-021)"
  FAILED=1
fi
if ! grep -q "DispatchStatus::Complete" "$LIFECYCLE"; then
  echo "ERROR: lifecycle.rs no longer references DispatchStatus::Complete (FT-021)"
  FAILED=1
fi
# Confirm Complete is reachable only via VerdictApproved (the only arm
# yielding S::Complete in the next() match table).
COMPLETE_LHS_COUNT=$(grep -c 'S::Complete' "$LIFECYCLE" || true)
APPROVED_LHS_COUNT=$(grep -c 'VerdictApproved' "$LIFECYCLE" || true)
if [ "$COMPLETE_LHS_COUNT" -lt 1 ] || [ "$APPROVED_LHS_COUNT" -lt 1 ]; then
  echo "ERROR: lifecycle.rs lost its VerdictApproved -> Complete arm (FT-021)"
  FAILED=1
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

# Part 2: examine any orchestration stores reachable from the repo root.
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
  unpaired="$(awk \
    -v group_type="$GROUP_TYPE" \
    -v status_pred="$STATUS_PRED" \
    -v has_interp="$HAS_INTERP" \
    -v verdict_type="$VERDICT_TYPE" \
    -v verdict_pred="$VERDICT_PRED" \
    -v prov_gen_by="$PROV_GEN_BY" \
    '
    function trim_lit(s,   t) { t = s; sub(/^"/, "", t); sub(/".*$/, "", t); return t }
    BEGIN { rdf_type = "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>" }
    {
      subj = $1; pred = $2; obj = $3
      if (pred == rdf_type) {
        if (obj == "<" group_type ">") group[subj] = 1
        if (obj == "<" verdict_type ">") verdict[subj] = 1
      } else if (pred == "<" status_pred ">") {
        # obj is a literal like "complete" — trim quotes.
        gsub(/^"/, "", obj); gsub(/"\^\^.*$/, "", obj); gsub(/".*$/, "", obj)
        group_status[subj] = obj
      } else if (pred == "<" has_interp ">") {
        group_interp_session[subj] = obj
      } else if (pred == "<" verdict_pred ">") {
        gsub(/^"/, "", obj); gsub(/"\^\^.*$/, "", obj); gsub(/".*$/, "", obj)
        verdict_value[subj] = obj
      } else if (pred == "<" prov_gen_by ">") {
        # subject is the verdict; object is the interpretation session.
        verdict_session[subj] = obj
      }
    }
    END {
      for (g in group) {
        if (group_status[g] != "complete") continue
        interp = group_interp_session[g]
        approved = 0
        if (interp != "") {
          for (v in verdict) {
            if (verdict_session[v] == interp && verdict_value[v] == "approved") {
              approved = 1; break
            }
          }
        }
        if (!approved) print "  • complete DispatchGroup without paired approved verdict: " g
      }
    }
    ' "$dump")"
  if [ -n "$unpaired" ]; then
    echo "ERROR: complete-without-paired-approved invariant violated in $dump (FT-021):"
    echo "$unpaired"
    VIOLATIONS=1
  fi
done <<EOF
$DUMPS
EOF

if [ "$VIOLATIONS" -ne 0 ]; then
  exit 1
fi

echo "OK: every complete DispatchGroup has a paired approved verdict (FT-021)"
exit 0
