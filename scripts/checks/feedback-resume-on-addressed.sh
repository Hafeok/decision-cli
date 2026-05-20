#!/usr/bin/env bash
# scripts/checks/feedback-resume-on-addressed.sh
#
# Enforces FT-032 / ADR-025 TC-037: a paused `dec:DispatchGroup`
# resumes ONLY after every blocking `dec:Feedback` in its
# `dec:blockedBy` set reaches a terminal lifecycle state.
#
# Concretely the rule has three arms:
#
#   - At least one non-terminal blocker → group stays in
#     `paused-for-feedback`. (Subscriptions retry naturally.)
#   - All blockers `addressed` (or `closed`) → group transitions back to
#     `awaiting-action` so the orchestrator can re-dispatch the action.
#   - Any blocker `rejected` → group transitions to
#     `feedback-rejected-action-blocked` (terminal failure).
#
# Three-part mechanical check:
#
#   1. Source invariant — `core::dispatch::lifecycle` declares the
#      `BlockingFeedbackAddressed` and `BlockingFeedbackRejected`
#      events AND maps them from `PausedForFeedback` to the correct
#      next states. The resume-subscription handler
#      (`core::subscriptions::feedback_resume`) AND the `resume_check`
#      API (`core::dispatch::pause::resume_check`) must both exist.
#
#   2. Subscription seed invariant — the `feedback_resume.ttl` seed is
#      loaded by `dec init` (extended in
#      `features/init/persist.rs::seed_bootstrap_subscriptions`).
#      Without the seed the subscription never fires.
#
#   3. Store invariant — every orchestration store with a paused
#      DispatchGroup whose `dec:blockedBy` set is entirely terminal
#      must NOT still carry `dec:dispatchStatus "paused-for-feedback"`.
#      If at least one blocker is rejected, the group MUST be in
#      `feedback-rejected-action-blocked`; otherwise the group must be
#      out of `paused-for-feedback` entirely.
#
# Exit 0: source machinery intact AND every paused group with all
#         blockers terminal has transitioned correctly.
# Exit 1: source / seed regression OR a stuck paused group.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

LIFECYCLE="crates/decision-cli/src/core/dispatch/lifecycle.rs"
PAUSE="crates/decision-cli/src/core/dispatch/pause.rs"
RESUME_SUB="crates/decision-cli/src/core/subscriptions/feedback_resume.rs"
RESUME_SEED="crates/decision-cli/src/core/subscriptions/seeds/feedback_resume.ttl"
INIT_PERSIST="crates/decision-cli/src/features/init/persist.rs"

FAILED=0
for f in "$LIFECYCLE" "$PAUSE" "$RESUME_SUB" "$RESUME_SEED" "$INIT_PERSIST"; do
  if [ ! -f "$f" ]; then
    echo "ERROR: expected $f (FT-032 anchor file)"
    FAILED=1
  fi
done
if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

# Part 1: lifecycle resume transitions.
if ! grep -q "BlockingFeedbackAddressed" "$LIFECYCLE"; then
  echo "ERROR: $LIFECYCLE no longer declares BlockingFeedbackAddressed (FT-032)"
  FAILED=1
fi
if ! grep -q "BlockingFeedbackRejected" "$LIFECYCLE"; then
  echo "ERROR: $LIFECYCLE no longer declares BlockingFeedbackRejected (FT-032)"
  FAILED=1
fi
if ! grep -q "FeedbackRejectedActionBlocked" "$LIFECYCLE"; then
  echo "ERROR: $LIFECYCLE no longer declares FeedbackRejectedActionBlocked (FT-032)"
  FAILED=1
fi
# PausedForFeedback + BlockingFeedbackAddressed → AwaitingAction
if ! grep -Pzq '\(S::PausedForFeedback, E::BlockingFeedbackAddressed\)[\s\S]*?=>[\s\S]*?S::AwaitingAction' "$LIFECYCLE" 2>/dev/null; then
  echo "ERROR: $LIFECYCLE no longer maps (PausedForFeedback, BlockingFeedbackAddressed) -> AwaitingAction (FT-032)"
  FAILED=1
fi
# PausedForFeedback + BlockingFeedbackRejected → FeedbackRejectedActionBlocked
if ! grep -Pzq '\(S::PausedForFeedback, E::BlockingFeedbackRejected\)[\s\S]*?=>[\s\S]*?S::FeedbackRejectedActionBlocked' "$LIFECYCLE" 2>/dev/null; then
  echo "ERROR: $LIFECYCLE no longer maps (PausedForFeedback, BlockingFeedbackRejected) -> FeedbackRejectedActionBlocked (FT-032)"
  FAILED=1
fi

# resume_check API present.
if ! grep -q "pub fn resume_check" "$PAUSE"; then
  echo "ERROR: $PAUSE no longer exports resume_check (FT-032)"
  FAILED=1
fi

# Resume subscription handler present.
if ! grep -q "pub fn handle_pending" "$RESUME_SUB"; then
  echo "ERROR: $RESUME_SUB no longer exports handle_pending (FT-032)"
  FAILED=1
fi
if ! grep -q "FEEDBACK_RESUME_HANDLER" "$RESUME_SUB"; then
  echo "ERROR: $RESUME_SUB no longer declares FEEDBACK_RESUME_HANDLER (FT-032)"
  FAILED=1
fi

# Resume subscription seed contents. The TTL embeds the SPARQL in a
# Turtle string literal — quotes inside the body are backslash-escaped.
for needle in 'dec:dispatchStatus.*paused-for-feedback' \
              'dec:blockedBy' \
              'dec:lifecycleState' \
              'addressed.*rejected.*closed'; do
  if ! grep -Eq "$needle" "$RESUME_SEED"; then
    echo "ERROR: $RESUME_SEED missing required clause: $needle (FT-032 §Behaviour)"
    FAILED=1
  fi
done

# Part 2: dec init wires the resume subscription seed.
if ! grep -q "feedback_resume::seed_quads" "$INIT_PERSIST"; then
  echo "ERROR: $INIT_PERSIST no longer seeds the feedback-resume subscription (FT-032)"
  FAILED=1
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

# Part 3: store invariant — no stuck paused group with all blockers terminal.
DUMPS="$(find . -path '*/.dec/store/orchestration.nq' -not -path '*/target/*' 2>/dev/null || true)"
if [ -z "$DUMPS" ]; then
  echo "OK: source + seed invariants intact; no orchestration stores to audit (vacuous PASS)"
  exit 0
fi

GROUP_TYPE="https://decision-cli.dev/ns#DispatchGroup"
STATUS_PRED="https://decision-cli.dev/ns#dispatchStatus"
BLOCKED_BY="https://decision-cli.dev/ns#blockedBy"
LIFECYCLE_STATE="https://decision-cli.dev/ns#lifecycleState"

VIOLATIONS=0
while IFS= read -r dump; do
  [ -z "$dump" ] && continue
  drift="$(awk \
    -v group_type="$GROUP_TYPE" \
    -v status_pred="$STATUS_PRED" \
    -v blocked_by="$BLOCKED_BY" \
    -v lifecycle_state="$LIFECYCLE_STATE" \
    '
    BEGIN { rdf_type = "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>" }
    function lit(o,  v) { v = o; gsub(/^"/, "", v); gsub(/"\^\^.*$/, "", v); gsub(/".*$/, "", v); return v }
    {
      subj = $1; pred = $2; obj = $3
      if (pred == rdf_type) {
        if (obj == "<" group_type ">") group[subj] = 1
      } else if (pred == "<" status_pred ">") {
        group_status[subj] = lit(obj)
      } else if (pred == "<" blocked_by ">") {
        n = ++group_blockers_n[subj]
        group_blocker[subj, n] = obj
      } else if (pred == "<" lifecycle_state ">") {
        feedback_state[subj] = lit(obj)
      }
    }
    END {
      for (g in group) {
        if (group_status[g] != "paused-for-feedback") continue
        n = group_blockers_n[g] + 0
        if (n == 0) continue   # caught by sibling script
        all_terminal = 1
        any_rejected = 0
        for (i = 1; i <= n; i++) {
          fb = group_blocker[g, i]
          state = feedback_state[fb]
          if (state == "rejected") { any_rejected = 1 }
          if (state != "addressed" && state != "closed" && state != "rejected" && state != "superseded") {
            all_terminal = 0
          }
        }
        if (!all_terminal) continue   # legitimately paused
        if (any_rejected) {
          # Should have transitioned to feedback-rejected-action-blocked.
          print "  • paused DispatchGroup " g " has a rejected blocker but still parks in paused-for-feedback (expected feedback-rejected-action-blocked)"
        } else {
          # Should have transitioned out of paused-for-feedback.
          print "  • paused DispatchGroup " g " has every blocker addressed/closed but still parks in paused-for-feedback (expected awaiting-action retry)"
        }
      }
    }
    ' "$dump")"
  if [ -n "$drift" ]; then
    echo "ERROR: resume-on-addressed invariant violated in $dump (FT-032 / TC-037):"
    echo "$drift"
    VIOLATIONS=1
  fi
done <<EOF
$DUMPS
EOF

if [ "$VIOLATIONS" -ne 0 ]; then
  exit 1
fi

echo "OK: paused dispatch resumes only after blocking feedback is terminal (FT-032 TC-037)"
exit 0
