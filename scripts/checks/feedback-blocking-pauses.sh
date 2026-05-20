#!/usr/bin/env bash
# scripts/checks/feedback-blocking-pauses.sh
#
# Enforces FT-032 / ADR-025 TC-036: when a worker emits blocking
# feedback, the orchestrator parks the paired `dec:DispatchGroup` in
# `dec:dispatchStatus "paused-for-feedback"` and refuses to advance to
# `awaiting-interpretation` until every blocking feedback is terminal.
#
# Three-part mechanical check:
#
#   1. Source invariant — `core::dispatch::lifecycle` declares
#      `PausedForFeedback` AND maps `BlockingFeedbackEmitted` from
#      `AwaitingAction` to `PausedForFeedback`. Drift in either arm
#      flips the central FT-032 claim.
#
#   2. API invariant — `core::dispatch::pause::pause_on_feedback` exists.
#      The implementer harness must call this helper rather than
#      bespoke quad writes, otherwise the `dec:blockedBy` invariant
#      cannot be audited.
#
#   3. Store invariant — every orchestration store at
#      `<workdir>/.dec/store/orchestration.nq` carrying a DispatchGroup
#      with `dec:dispatchStatus "paused-for-feedback"` MUST also carry
#      at least one `dec:blockedBy` link to a `dec:Feedback`. A paused
#      group with no blocker is a regression (the SHACL shape forbids
#      it; the script is the belt-and-braces audit).
#
# Exit 0: source machinery intact AND every paused group references a
#         blocking feedback.
# Exit 1: source machinery regressed OR a paused group is missing
#         `dec:blockedBy`.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

LIFECYCLE="crates/decision-cli/src/core/dispatch/lifecycle.rs"
PAUSE="crates/decision-cli/src/core/dispatch/pause.rs"
HARNESS_DIR="crates/decision-cli/src/features/implement"

FAILED=0

for f in "$LIFECYCLE" "$PAUSE"; do
  if [ ! -f "$f" ]; then
    echo "ERROR: expected $f (FT-032 anchor file)"
    FAILED=1
  fi
done
if [ ! -d "$HARNESS_DIR" ]; then
  echo "ERROR: expected $HARNESS_DIR (FT-032 harness root)"
  FAILED=1
fi
if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

# Part 1: lifecycle state machine.
if ! grep -q "PausedForFeedback" "$LIFECYCLE"; then
  echo "ERROR: $LIFECYCLE no longer declares PausedForFeedback (FT-032)"
  FAILED=1
fi
if ! grep -q "BlockingFeedbackEmitted" "$LIFECYCLE"; then
  echo "ERROR: $LIFECYCLE no longer declares BlockingFeedbackEmitted (FT-032)"
  FAILED=1
fi
# AwaitingAction + BlockingFeedbackEmitted → PausedForFeedback transition.
if ! grep -Pzq '\(S::AwaitingAction, E::BlockingFeedbackEmitted\)[\s\S]*?=>[\s\S]*?S::PausedForFeedback' "$LIFECYCLE" 2>/dev/null; then
  echo "ERROR: $LIFECYCLE no longer maps (AwaitingAction, BlockingFeedbackEmitted) -> PausedForFeedback (FT-032)"
  FAILED=1
fi

# Part 2: pause_on_feedback API + dec:blockedBy quad builder.
if ! grep -q "pub fn pause_on_feedback" "$PAUSE"; then
  echo "ERROR: $PAUSE no longer exports pause_on_feedback (FT-032)"
  FAILED=1
fi
if ! grep -q "build_blocked_by_quad" "$PAUSE"; then
  echo "ERROR: $PAUSE no longer references build_blocked_by_quad (FT-032)"
  FAILED=1
fi

# Part 3: implementer harness invokes pause_on_feedback. The call can
# live in any file under features/implement/ (mod.rs or a split-out
# submodule like feedback_handling.rs).
if ! grep -rq "pause_on_feedback" "$HARNESS_DIR"; then
  echo "ERROR: $HARNESS_DIR no longer wires pause_on_feedback (FT-032 §Behaviour step 6)"
  FAILED=1
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

# Part 4: store invariant — every paused group must carry at least one
# dec:blockedBy. Vacuous PASS when no orchestration store is reachable.
DUMPS="$(find . -path '*/.dec/store/orchestration.nq' -not -path '*/target/*' 2>/dev/null || true)"
if [ -z "$DUMPS" ]; then
  echo "OK: source invariants intact; no orchestration stores to audit (vacuous PASS)"
  exit 0
fi

GROUP_TYPE="https://decision-cli.dev/ns#DispatchGroup"
STATUS_PRED="https://decision-cli.dev/ns#dispatchStatus"
BLOCKED_BY="https://decision-cli.dev/ns#blockedBy"

VIOLATIONS=0
while IFS= read -r dump; do
  [ -z "$dump" ] && continue
  drift="$(awk \
    -v group_type="$GROUP_TYPE" \
    -v status_pred="$STATUS_PRED" \
    -v blocked_by="$BLOCKED_BY" \
    '
    BEGIN { rdf_type = "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>" }
    {
      subj = $1; pred = $2; obj = $3
      if (pred == rdf_type) {
        if (obj == "<" group_type ">") group[subj] = 1
      } else if (pred == "<" status_pred ">") {
        gsub(/^"/, "", obj); gsub(/"\^\^.*$/, "", obj); gsub(/".*$/, "", obj)
        group_status[subj] = obj
      } else if (pred == "<" blocked_by ">") {
        group_has_blocker[subj] = 1
      }
    }
    END {
      for (g in group) {
        if (group_status[g] != "paused-for-feedback") continue
        if (!group_has_blocker[g]) {
          print "  • paused DispatchGroup " g " has no dec:blockedBy link"
        }
      }
    }
    ' "$dump")"
  if [ -n "$drift" ]; then
    echo "ERROR: paused-for-feedback invariant violated in $dump (FT-032 / TC-036):"
    echo "$drift"
    VIOLATIONS=1
  fi
done <<EOF
$DUMPS
EOF

if [ "$VIOLATIONS" -ne 0 ]; then
  exit 1
fi

echo "OK: blocking feedback pauses the emitting dispatch (FT-032 TC-036)"
exit 0
