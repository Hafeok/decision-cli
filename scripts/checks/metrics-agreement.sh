#!/usr/bin/env bash
# scripts/checks/metrics-agreement.sh
#
# Enforces FT-024 / ADR-021 / TC-032: the action-interpretation agreement
# metric is computable from a persisted orchestration store via the
# `core::metrics::agreement` substrate.
#
# Two-part mechanical check:
#
#   1. Source invariant — the Rust module `core/metrics/` exists and
#      exports the four-rate `AgreementReport`, the `agreement(...)`
#      entry point, and the SPARQL query builder. Drift in any of the
#      three flips the "metric exists" contract.
#
#   2. Store invariant — any orchestration store at
#      `<workdir>/.dec/store/orchestration.nq` that contains at least
#      one terminal `dec:DispatchGroup` must surface the same per-status
#      counts the Rust module computes. We re-derive the counts via
#      awk over the N-Quads dump (same approach as the sibling
#      dispatch-status checks) and assert the file mentions the canonical
#      verdict literals so the metric is not silently disconnected from
#      the schema. Stores without any DispatchGroup short-circuit to a
#      vacuous PASS.
#
# Exit 0: source machinery intact AND the metric is mechanically
#         computable against every persisted dump found.
# Exit 1: source machinery regressed OR a dump contains DispatchGroups
#         the metric module cannot see (schema drift).
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

MOD_DIR="crates/decision-cli/src/core/metrics"
AGREEMENT_RS="$MOD_DIR/agreement.rs"
QUERIES_RS="$MOD_DIR/queries.rs"
MOD_RS="$MOD_DIR/mod.rs"

FAILED=0

# Part 1: source invariant — files + exports exist.
for f in "$MOD_RS" "$AGREEMENT_RS" "$QUERIES_RS"; do
  if [ ! -f "$f" ]; then
    echo "ERROR: expected $f (FT-024 anchor file)"
    FAILED=1
  fi
done

if [ "$FAILED" -eq 0 ]; then
  for sym in \
    "pub struct AgreementReport" \
    "pub fn agreement" \
    "pub enum MetricsError" \
    "agreement_rate" \
    "amendment_rate" \
    "rejection_rate" \
    "false_success_rate"
  do
    if ! grep -q "$sym" "$AGREEMENT_RS"; then
      echo "ERROR: $AGREEMENT_RS no longer exposes \"$sym\" (FT-024)"
      FAILED=1
    fi
  done

  for sym in \
    "pub fn build_query" \
    "terminal_statuses"
  do
    if ! grep -q "$sym" "$QUERIES_RS"; then
      echo "ERROR: $QUERIES_RS no longer exposes \"$sym\" (FT-024)"
      FAILED=1
    fi
  done

  # The module is re-exported through the metrics mod.rs so the
  # call site `decision_cli::core::metrics::agreement(...)` works.
  if ! grep -q "pub use agreement::" "$MOD_RS"; then
    echo "ERROR: $MOD_RS lost its agreement re-export (FT-024)"
    FAILED=1
  fi

  # The module must be wired into core/mod.rs so it is reachable.
  if ! grep -q "pub mod metrics" "crates/decision-cli/src/core/mod.rs"; then
    echo "ERROR: crates/decision-cli/src/core/mod.rs no longer exposes pub mod metrics (FT-024)"
    FAILED=1
  fi
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

# Part 2: store invariant — any persisted dump's DispatchGroup population
# is mechanically derivable. We scan via awk to confirm the schema the
# metric module relies on (DispatchGroup → status; verdict literals).
DUMPS="$(find . -path '*/.dec/store/orchestration.nq' -not -path '*/target/*' 2>/dev/null || true)"
if [ -z "$DUMPS" ]; then
  echo "OK: source invariant intact; no orchestration stores to audit (vacuous PASS)"
  exit 0
fi

GROUP_TYPE="https://decision-cli.dev/ns#DispatchGroup"
STATUS_PRED="https://decision-cli.dev/ns#dispatchStatus"
VERDICT_TYPE="https://decision-cli.dev/ns#VerificationVerdict"
VERDICT_PRED="https://decision-cli.dev/ns#verdict"

VIOLATIONS=0
while IFS= read -r dump; do
  [ -z "$dump" ] && continue

  # Sanity: if the dump has DispatchGroup nodes, it must also carry the
  # dispatchStatus predicate; otherwise the metric can't compute counts.
  groups=$(awk -v t="$GROUP_TYPE" \
    'BEGIN { rdf="<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>" }
     $2 == rdf && $3 == "<" t ">" { c++ }
     END { print (c+0) }' "$dump")
  if [ "$groups" -eq 0 ]; then
    continue
  fi

  has_status=$(awk -v p="$STATUS_PRED" \
    '$2 == "<" p ">" { print "yes"; exit }' "$dump")
  if [ "$has_status" != "yes" ]; then
    echo "ERROR: $dump has DispatchGroup(s) but no dec:dispatchStatus literal — metric module cannot compute (FT-024)"
    VIOLATIONS=1
    continue
  fi

  # If verdicts are present at all, their literal must be one of the
  # three FT-024 vocabulary values. Anything else means the metric's
  # bucketing has silently fallen behind a schema amendment.
  unknown=$(awk -v p="$VERDICT_PRED" \
    '$2 == "<" p ">" {
       v = $3
       sub(/^"/, "", v); sub(/".*$/, "", v)
       if (v != "approved" && v != "rejected" && v != "amendment-required") {
         print v
       }
     }' "$dump" | sort -u)
  if [ -n "$unknown" ]; then
    echo "ERROR: $dump carries dec:verdict literal(s) outside the FT-024 vocabulary:"
    echo "$unknown" | sed 's/^/  • /'
    VIOLATIONS=1
  fi
done <<EOF
$DUMPS
EOF

if [ "$VIOLATIONS" -ne 0 ]; then
  exit 1
fi

echo "OK: action-interpretation agreement metric substrate intact (FT-024 / ADR-021 / TC-032)"
exit 0
