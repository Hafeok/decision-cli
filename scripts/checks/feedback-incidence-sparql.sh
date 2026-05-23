#!/usr/bin/env bash
# scripts/checks/feedback-incidence-sparql.sh
#
# Enforces FT-026 / TC-040 — aggregate feedback class incidence must be
# computable from the orchestration store alone via a SPARQL query
# against the schema FT-026 lands.
#
# Two-part mechanical check:
#
#   1. Source invariant — `core/feedback/` exists, exposes the `Feedback`
#      shape with `to_quads` + `from_quads`-style read helpers, and the
#      `dec:feedbackClass` predicate is declared in the embedded ontology
#      and bound by the `dec:FeedbackShape` SHACL targetClass.
#
#   2. Store invariant — for any persisted dump
#      `<workdir>/.dec/store/orchestration.nq` containing at least one
#      `dec:Feedback` node, every such node carries the
#      `dec:feedbackClass` literal needed to bucket incidence. A dump
#      with `dec:Feedback` nodes missing the class literal would defeat
#      the metric. Stores without `dec:Feedback` short-circuit to a
#      vacuous PASS — FT-026 ships the schema, FT-027/FT-028/FT-029
#      ship the production paths that populate it.
#
# Exit 0: schema substrate intact AND every persisted feedback dump's
#         population is SPARQL-aggregable by class.
# Exit 1: source machinery regressed OR a dump's feedback population
#         cannot be bucketed by class (schema drift).
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

MOD_DIR="crates/decision-cli/src/core/feedback"
ARTIFACT_RS="$MOD_DIR/artifact.rs"
READ_RS="$MOD_DIR/read.rs"
SHACL_RS="$MOD_DIR/shacl.rs"
MOD_RS="$MOD_DIR/mod.rs"
ONTOLOGY_TTL="crates/decision-cli/src/core/ontology/assets/ontology.ttl"
SHAPES_TTL="crates/decision-cli/src/core/ontology/assets/shapes.ttl"
# vocab.rs was split into a module directory (core/vocab/<topic>.rs)
# to stay under the ADR-013 400-line cap; the feedback-related IRIs
# now live in core/vocab/feedback.rs and are re-exported via mod.rs.
VOCAB_RS="crates/decision-cli/src/core/vocab/feedback.rs"

FAILED=0

# --- Part 1: source invariant -------------------------------------------------
for f in "$MOD_RS" "$ARTIFACT_RS" "$READ_RS" "$SHACL_RS"; do
  if [ ! -f "$f" ]; then
    echo "ERROR: expected $f (FT-026 anchor file)"
    FAILED=1
  fi
done

if [ "$FAILED" -eq 0 ]; then
  for sym in \
    "pub struct Feedback" \
    "pub fn to_quads"
  do
    if ! grep -q "$sym" "$ARTIFACT_RS"; then
      echo "ERROR: $ARTIFACT_RS no longer exposes \"$sym\" (FT-026)"
      FAILED=1
    fi
  done

  for sym in \
    "pub fn get" \
    "pub fn list_open" \
    "pub fn list_by_class" \
    "pub fn list_by_target" \
    "pub enum FeedbackReadError"
  do
    if ! grep -q "$sym" "$READ_RS"; then
      echo "ERROR: $READ_RS no longer exposes \"$sym\" (FT-026)"
      FAILED=1
    fi
  done

  if ! grep -q "pub fn validate_quads" "$SHACL_RS"; then
    echo "ERROR: $SHACL_RS no longer exposes pub fn validate_quads (FT-026)"
    FAILED=1
  fi

  if ! grep -q "pub mod feedback" "crates/decision-cli/src/core/mod.rs"; then
    echo "ERROR: crates/decision-cli/src/core/mod.rs no longer exposes pub mod feedback (FT-026)"
    FAILED=1
  fi

  if ! grep -q "validate_feedback" "crates/decision-cli/src/core/stream_writer.rs"; then
    echo "ERROR: StreamWriter no longer validates Feedback mutations (FT-026)"
    FAILED=1
  fi

  if ! grep -q "dec:Feedback" "$ONTOLOGY_TTL"; then
    echo "ERROR: ontology.ttl no longer declares dec:Feedback (FT-026)"
    FAILED=1
  fi

  if ! grep -q "dec:feedbackClass" "$ONTOLOGY_TTL"; then
    echo "ERROR: ontology.ttl no longer declares dec:feedbackClass (FT-026)"
    FAILED=1
  fi

  if ! grep -q "dec:FeedbackShape" "$SHAPES_TTL"; then
    echo "ERROR: shapes.ttl no longer declares dec:FeedbackShape (FT-026)"
    FAILED=1
  fi

  if ! grep -q "sh:targetClass dec:Feedback" "$SHAPES_TTL"; then
    echo "ERROR: dec:FeedbackShape no longer targets dec:Feedback (FT-026)"
    FAILED=1
  fi

  if ! grep -q "IRI_DEC_FEEDBACK_CLASS" "$VOCAB_RS"; then
    echo "ERROR: vocab.rs no longer exports IRI_DEC_FEEDBACK_CLASS (FT-026)"
    FAILED=1
  fi
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

# --- Part 2: store invariant --------------------------------------------------
DUMPS="$(find . -path '*/.dec/store/orchestration.nq' -not -path '*/target/*' 2>/dev/null || true)"
if [ -z "$DUMPS" ]; then
  echo "OK: schema substrate intact; no orchestration stores to audit (vacuous PASS)"
  exit 0
fi

FEEDBACK_TYPE="https://decision-cli.dev/ns#Feedback"
CLASS_PRED="https://decision-cli.dev/ns#feedbackClass"
RDF_TYPE="http://www.w3.org/1999/02/22-rdf-syntax-ns#type"

VIOLATIONS=0
while IFS= read -r dump; do
  [ -z "$dump" ] && continue

  feedback_count=$(awk -v t="$FEEDBACK_TYPE" -v rdf="$RDF_TYPE" \
    '$2 == "<" rdf ">" && $3 == "<" t ">" { c++ }
     END { print (c+0) }' "$dump")
  if [ "$feedback_count" -eq 0 ]; then
    continue
  fi

  with_class=$(awk -v p="$CLASS_PRED" \
    '$2 == "<" p ">" { c++ }
     END { print (c+0) }' "$dump")
  if [ "$with_class" -lt "$feedback_count" ]; then
    echo "ERROR: $dump has $feedback_count dec:Feedback node(s) but only $with_class dec:feedbackClass literal(s) — incidence aggregation broken (FT-026)"
    VIOLATIONS=1
  fi
done <<EOF
$DUMPS
EOF

if [ "$VIOLATIONS" -ne 0 ]; then
  exit 1
fi

echo "OK: feedback class incidence is SPARQL-aggregable (FT-026 / TC-040)"
exit 0
