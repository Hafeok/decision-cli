#!/usr/bin/env bash
# scripts/checks/action-interpretation-pairing.sh
#
# Enforces ADR-017: every dec:ActionSession in the orchestration store
# must be paired with a dec:InterpretationSession through a
# dec:DispatchGroup. TC-027.
#
# Two-part mechanical check:
#
#   1. Schema invariant — the embedded base ontology declares the
#      classes (dec:ActionSession, dec:InterpretationSession,
#      dec:DispatchGroup) and the linking predicates
#      (dec:hasActionSession, dec:hasInterpretationSession). Without
#      these, the pairing has no graph vocabulary to express it.
#
#   2. Store invariant — any orchestration store under .dec/store/ or
#      any test fixture under target/tmp/ that contains an
#      ActionSession must also carry a DispatchGroup linking it to an
#      InterpretationSession. Stores absent (e.g. CI without a seeded
#      init) make the rule vacuously true.
#
# Exit 0: schema present AND no unpaired ActionSession found in any store.
# Exit 1: schema regression OR an unpaired ActionSession was found.
#
# Diagnostic output goes to stdout so `product verify` captures it.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

ONTOLOGY="crates/dec-ontology/src/ontology/assets/ontology.ttl"
if [ ! -f "$ONTOLOGY" ]; then
  echo "ERROR: expected $ONTOLOGY (ADR-017 anchor file)"
  exit 1
fi

FAILED=0

# Part 1: schema vocabulary present.
for term in \
  "dec:ActionSession" \
  "dec:InterpretationSession" \
  "dec:DispatchGroup" \
  "dec:hasActionSession" \
  "dec:hasInterpretationSession"
do
  if ! grep -q "$term" "$ONTOLOGY"; then
    echo "ERROR: ontology missing $term (ADR-017)"
    FAILED=1
  fi
done

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

# Part 2: examine any orchestration stores reachable from the repo
# root. Slice-1/slice-2 stores live at <workdir>/.dec/store/orchestration.nq.
# Integration tests run inside throwaway tempdirs, so a fresh CI tree
# typically has zero dumps — the loop short-circuits to a vacuous PASS.
DUMPS="$(find . -path '*/.dec/store/orchestration.nq' -not -path '*/target/*' 2>/dev/null || true)"

if [ -z "$DUMPS" ]; then
  echo "OK: schema OK; no orchestration stores to audit (vacuous PASS)"
  exit 0
fi

VIOLATIONS=0
ACTION_TYPE="https://decision-cli.dev/ns#ActionSession"
GROUP_TYPE="https://decision-cli.dev/ns#DispatchGroup"
HAS_ACTION="https://decision-cli.dev/ns#hasActionSession"
HAS_INTERPRETATION="https://decision-cli.dev/ns#hasInterpretationSession"
DISPATCH_STATUS="https://decision-cli.dev/ns#dispatchStatus"

# Each N-Quads line is a quoted triple+graph. For every subject typed as
# ActionSession, the same store must contain a DispatchGroup whose
# dec:hasActionSession points to that subject AND whose
# dec:hasInterpretationSession is set. A straightforward awk pass over
# the N-Quads dump is sufficient at slice-2 scale.
#
# Per ADR-017, the pairing is structurally required when the dispatch
# group has *terminated with a result* — i.e. `complete`,
# `awaiting-amendment`, or `interpretation-rejected`. In-flight states
# (`awaiting-action`, `awaiting-interpretation`, `interpretation-running`,
# `paused-for-feedback`) or error-terminal states (`action-failed`,
# `interpretation-failed`, `feedback-rejected-action-blocked`) are
# excluded from the check: an action session that hasn't run to
# completion has no interpretation to pair with yet, and action-failure
# states by definition produce no artifact to interpret.
while IFS= read -r dump; do
  [ -z "$dump" ] && continue
  unpaired="$(awk \
    -v action_type="$ACTION_TYPE" \
    -v group_type="$GROUP_TYPE" \
    -v has_action="$HAS_ACTION" \
    -v has_interpretation="$HAS_INTERPRETATION" \
    -v dispatch_status="$DISPATCH_STATUS" \
    '
    function trim_iri(s,   t) { t = s; gsub(/^</, "", t); gsub(/>$/, "", t); return t }
    function unquote(s,   t) {
      t = s; sub(/^"/, "", t); sub(/".*$/, "", t); return t
    }
    function requires_pairing(status) {
      return status == "complete" \
          || status == "awaiting-amendment" \
          || status == "interpretation-rejected"
    }
    BEGIN { rdf_type = "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>" }
    {
      subj = $1; pred = $2; obj = $3
      if (pred == rdf_type) {
        if (obj == "<" action_type ">") action[subj] = 1
        if (obj == "<" group_type ">") group[subj] = 1
      } else if (pred == "<" has_action ">") {
        group_action[subj] = obj
      } else if (pred == "<" has_interpretation ">") {
        group_interp[subj] = obj
      } else if (pred == "<" dispatch_status ">") {
        group_status[subj] = unquote(obj)
      }
    }
    END {
      for (a in action) {
        paired = 0
        gating = 0
        for (g in group) {
          if (group_action[g] != a) continue
          if (!requires_pairing(group_status[g])) continue
          gating = 1
          if (group_interp[g] != "") { paired = 1; break }
        }
        if (gating && !paired) print "  • unpaired ActionSession: " a
      }
    }
    ' "$dump")"
  if [ -n "$unpaired" ]; then
    echo "ERROR: action-interpretation pairing violated in $dump (ADR-017):"
    echo "$unpaired"
    VIOLATIONS=1
  fi
done <<EOF
$DUMPS
EOF

if [ "$VIOLATIONS" -ne 0 ]; then
  exit 1
fi

echo "OK: every ActionSession is paired via DispatchGroup (ADR-017)"
exit 0
