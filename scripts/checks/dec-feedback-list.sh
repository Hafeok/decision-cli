#!/usr/bin/env bash
# scripts/checks/dec-feedback-list.sh
#
# Enforces FT-029 / TC-039 — `dec feedback list` returns the open
# feedback corpus grouped by class and target role.
#
# Two-part mechanical check:
#
#   1. Source invariant — `core/feedback/routing/` exists with the
#      ADR-026 routing table (`table.rs`) and the delivery handler
#      (`handler.rs`), the feature surface `features/feedback/` is
#      wired to read `list_open` from `core/feedback/read.rs`, and the
#      CLI dispatch reaches `feedback::list`. Drift in any of these
#      unhooks the routing layer from the read surface.
#
#   2. Behavioural invariant — initialise a throwaway working tree,
#      seed three feedback artifacts in two classes + two targets, and
#      assert `dec feedback list` groups them by `class:` then
#      `target:` headers per FT-029. The script also asserts that a
#      feedback marked with `dec:rejectionReason` (the FT-029
#      routing-rejection path) is excluded from the open list.
#
# Exit 0: source machinery intact AND the seeded list renders correctly.
# Exit 1: source machinery regressed OR the rendered list disagrees.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

ROUTING_DIR="crates/decision-cli/src/core/feedback/routing"
TABLE_RS="$ROUTING_DIR/table.rs"
HANDLER_RS="$ROUTING_DIR/handler.rs"
SEED_TTL="crates/decision-cli/src/core/feedback/seeds/feedback_routing.ttl"
FEATURE_MOD="crates/decision-cli/src/features/feedback/mod.rs"
CLI_MOD="crates/decision-cli/src/cli/feedback.rs"
# main.rs was reduced to a dispatch-only entry point per ADR-013; the
# top-level subcommand enum lives in cli/args.rs after the split.
ARGS_RS="crates/decision-cli/src/cli/args.rs"
INIT_PERSIST="crates/decision-cli/src/features/init/persist.rs"
# `seed_quads` was extracted from handler.rs into routing/seed.rs to
# stay under the ADR-013 400-line cap; routing/mod.rs re-exports it.
ROUTING_MOD="$ROUTING_DIR/mod.rs"

FAILED=0

# --- Part 1: source invariant -------------------------------------------------
for f in "$TABLE_RS" "$HANDLER_RS" "$SEED_TTL" "$FEATURE_MOD" "$CLI_MOD"; do
  if [ ! -f "$f" ]; then
    echo "ERROR: expected $f (FT-029 anchor file)"
    FAILED=1
  fi
done

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

for sym in \
  "pub const ROUTING_TABLE" \
  "pub fn rule_for" \
  "pub fn default_target_role"
do
  if ! grep -q "$sym" "$TABLE_RS"; then
    echo "ERROR: $TABLE_RS no longer exposes \"$sym\" (FT-029)"
    FAILED=1
  fi
done

for sym in \
  "pub fn pending_feedback" \
  "pub fn route_pending_feedback" \
  "FEEDBACK_ROUTING_SUBSCRIPTION_IRI"
do
  if ! grep -q "$sym" "$HANDLER_RS"; then
    echo "ERROR: $HANDLER_RS no longer exposes \"$sym\" (FT-029)"
    FAILED=1
  fi
done

# seed_quads moved to its own file under the same module; routing/mod.rs
# re-exports it. Accept either location.
if ! grep -Eq "pub (use seed::seed_quads|fn seed_quads)" "$ROUTING_MOD" "$ROUTING_DIR"/seed.rs 2>/dev/null; then
  echo "ERROR: routing module no longer exposes seed_quads via $ROUTING_MOD or seed.rs (FT-029)"
  FAILED=1
fi

if ! grep -q "feedback-routing" "$SEED_TTL"; then
  echo "ERROR: $SEED_TTL no longer declares the feedback-routing handler tag (FT-029)"
  FAILED=1
fi
if ! grep -q "dec:lifecycleState" "$SEED_TTL"; then
  echo "ERROR: $SEED_TTL no longer matches dec:lifecycleState produced (FT-029)"
  FAILED=1
fi

if ! grep -q "pub fn list" "$FEATURE_MOD"; then
  echo "ERROR: $FEATURE_MOD no longer exposes pub fn list (FT-029)"
  FAILED=1
fi
if ! grep -q "pub fn format_list" "$FEATURE_MOD"; then
  echo "ERROR: $FEATURE_MOD no longer exposes pub fn format_list (FT-029)"
  FAILED=1
fi

if ! grep -q "Feedback(FeedbackCmd)" "$ARGS_RS"; then
  echo "ERROR: $ARGS_RS no longer dispatches the Feedback subcommand (FT-029)"
  FAILED=1
fi
if ! grep -q "feedback::list" "$CLI_MOD"; then
  echo "ERROR: $CLI_MOD no longer wires the feedback::list call (FT-029)"
  FAILED=1
fi

if ! grep -q "feedback::routing::seed_quads" "$INIT_PERSIST"; then
  echo "ERROR: $INIT_PERSIST no longer seeds the feedback-routing subscription (FT-029)"
  FAILED=1
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

# --- Part 2: behavioural invariant -------------------------------------------
TMPROOT="$(mktemp -d -t ft029.XXXXXX)"
trap 'rm -rf "$TMPROOT"' EXIT

WORKDIR="$TMPROOT/work"
mkdir -p "$WORKDIR"

# Build the binary up-front so the run steps below are fast and don't
# emit cargo noise into the asserted output.
cargo build --quiet --package decision-cli --bin dec
DEC="./target/debug/dec"
if [ ! -x "$DEC" ]; then
  echo "ERROR: built binary $DEC not found"
  exit 1
fi

"$DEC" --workdir "$WORKDIR" init --template engineering-development >/dev/null

DUMP="$WORKDIR/.dec/store/orchestration.nq"
if [ ! -f "$DUMP" ]; then
  echo "ERROR: dec init did not produce $DUMP"
  exit 1
fi

# Resolve the active stream IRI — every Feedback artifact must carry
# dec:inStream to that IRI for the read API to surface it.
STREAM_IRI="$(awk '
  /<https:\/\/decision-cli\.dev\/ns#ValueStream>/ {
    # Subject is the first column; strip < >.
    sub(/^</, "", $1); sub(/>$/, "", $1); print $1; exit
  }' "$DUMP")"
if [ -z "$STREAM_IRI" ]; then
  echo "ERROR: could not locate a dec:ValueStream IRI in $DUMP"
  exit 1
fi

ORCH_GRAPH="<https://decision-cli.dev/ns/orchestration>"
DEC_NS="https://decision-cli.dev/ns#"

emit_feedback() {
  local iri="$1" class="$2" target="$3" state="$4" evidence="$5" reject_reason="${6:-}"
  cat >> "$DUMP" <<EOF
<$iri> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <${DEC_NS}Feedback> $ORCH_GRAPH .
<$iri> <${DEC_NS}feedbackClass> "$class" $ORCH_GRAPH .
<$iri> <${DEC_NS}lifecycleState> "$state" $ORCH_GRAPH .
<$iri> <${DEC_NS}targetRole> "$target" $ORCH_GRAPH .
<$iri> <${DEC_NS}evidence> "$evidence" $ORCH_GRAPH .
<$iri> <${DEC_NS}severity> "warning" $ORCH_GRAPH .
<$iri> <${DEC_NS}sourceSession> <https://decision-cli.dev/ns/session/seeded-1> $ORCH_GRAPH .
<$iri> <${DEC_NS}inStream> <$STREAM_IRI> $ORCH_GRAPH .
EOF
  if [ -n "$reject_reason" ]; then
    cat >> "$DUMP" <<EOF
<$iri> <${DEC_NS}rejectionReason> "$reject_reason" $ORCH_GRAPH .
EOF
  fi
}

emit_feedback "urn:dec:test:feedback:1" "gap" "spec-author" "produced" \
  "feature_spec FT-029 line 42 underspecifies the rejection path"
emit_feedback "urn:dec:test:feedback:2" "gap" "spec-author" "routed" \
  "follow-up gap on the same spec"
emit_feedback "urn:dec:test:feedback:3" "contradiction" "architect" "produced" \
  "ADR-024 vs FT-029 disagree on produced to rejected transition"
# Feedback whose routing failed — must NOT appear in `list`.
emit_feedback "urn:dec:test:feedback:4" "gap" "spec-author" "produced" \
  "should not surface" "unknown-target-role"
# Terminal feedback — also must NOT appear.
emit_feedback "urn:dec:test:feedback:5" "defect" "verifier" "closed" \
  "already closed"

LIST_OUT="$("$DEC" --workdir "$WORKDIR" feedback list)"

# The renderer groups by class first, target second. Verify the structure.
expect_line() {
  local needle="$1"
  if ! printf '%s\n' "$LIST_OUT" | grep -Fxq "$needle"; then
    echo "ERROR: expected line in 'dec feedback list' output: $needle"
    echo "--- actual output ---"
    printf '%s\n' "$LIST_OUT"
    echo "--- end output ---"
    FAILED=1
  fi
}

expect_line "class: contradiction"
expect_line "  target: architect"
expect_line "class: gap"
expect_line "  target: spec-author"

for iri in "urn:dec:test:feedback:1" "urn:dec:test:feedback:2" "urn:dec:test:feedback:3"; do
  if ! printf '%s\n' "$LIST_OUT" | grep -q "$iri"; then
    echo "ERROR: expected $iri in 'dec feedback list' output"
    FAILED=1
  fi
done

for hidden in "urn:dec:test:feedback:4" "urn:dec:test:feedback:5"; do
  if printf '%s\n' "$LIST_OUT" | grep -q "$hidden"; then
    echo "ERROR: $hidden must be excluded from the open list"
    FAILED=1
  fi
done

# Class ordering is alphabetical: contradiction before gap.
contradiction_line="$(printf '%s\n' "$LIST_OUT" | grep -n '^class: contradiction$' | head -1 | cut -d: -f1)"
gap_line="$(printf '%s\n' "$LIST_OUT" | grep -n '^class: gap$' | head -1 | cut -d: -f1)"
if [ -n "$contradiction_line" ] && [ -n "$gap_line" ]; then
  if [ "$contradiction_line" -ge "$gap_line" ]; then
    echo "ERROR: 'contradiction' must group before 'gap' in the list output"
    FAILED=1
  fi
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

echo "OK: dec feedback list groups open feedback by class and target (FT-029 / TC-039)"
exit 0
