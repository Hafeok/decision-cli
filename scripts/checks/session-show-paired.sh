#!/usr/bin/env bash
# scripts/checks/session-show-paired.sh
#
# Enforces FT-025 / ADR-017 / ADR-018 / TC-031: when a Session is part of
# a `dec:DispatchGroup`, `dec session show <iri>` appends a "Paired:"
# block carrying the group's status, the paired session IRI, and — if a
# verifier `dec:VerificationVerdict` exists — the verdict value, its
# rationale, the `dec:violates` list, and the `dec:amendmentGuidance`
# text.
#
# Two-part mechanical check:
#
#   1. Source invariant — the paired-display module exists and references
#      the FT-021 / ADR-018 vocabulary it must surface (DispatchGroup,
#      hasInterpretationSession, VerificationVerdict, dec:verdict,
#      dec:rationale, dec:violates, dec:amendmentGuidance, Paired:
#      header). Drift in any of these unhooks the CLI surface from the
#      ontology.
#
#   2. Behavioural invariant — the FT-025 integration test exercises
#      `session_show` against a synthetic store seeded with the same
#      RDF shape `dec implement` + the verifier worker would write,
#      and asserts the rendered output carries the Paired: block, the
#      verdict, the rationale, and (for amendment-required) the
#      violates list + guidance. We delegate to `cargo test` so the
#      check stays a single, falsifiable end-to-end assertion.
#
# Exit 0: source machinery intact AND the integration test passes.
# Exit 1: source machinery regressed OR the integration test failed.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

SESSION_SHOW="crates/decision-cli/src/features/implement/session_show.rs"
PAIRED="crates/decision-cli/src/features/implement/session_show/paired.rs"
TEST_FILE="crates/decision-cli/tests/ft_025_session_show_paired.rs"
TEST_DATA="crates/decision-cli/tests/data/ft_025_paired_session.nq"

FAILED=0

for f in "$SESSION_SHOW" "$PAIRED" "$TEST_FILE" "$TEST_DATA"; do
  if [ ! -f "$f" ]; then
    echo "ERROR: expected $f (FT-025 anchor file)"
    FAILED=1
  fi
done

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

# Source invariant: the paired-display module names the vocabulary it
# is contractually obligated to surface. Each token is a load-bearing
# reference into the FT-021 / ADR-018 schema.
for token in \
  "DispatchGroup" \
  "hasActionSession" \
  "hasInterpretationSession" \
  "VerificationVerdict" \
  "dec:verdict" \
  "dec:rationale" \
  "dec:violates" \
  "dec:amendmentGuidance" \
  "dec:dispatchStatus" \
  "Paired:" \
  "Verdict:" \
  "Rationale:" \
  "Dispatch group:"
do
  if ! grep -q -- "$token" "$PAIRED"; then
    echo "ERROR: $PAIRED no longer references \"$token\" (FT-025 / TC-031)"
    FAILED=1
  fi
done

# The session_show.rs entry point must wire the paired module in.
if ! grep -q "mod paired" "$SESSION_SHOW"; then
  echo "ERROR: $SESSION_SHOW no longer wires the paired display module (FT-025)"
  FAILED=1
fi
if ! grep -q "lookup_paired_block" "$SESSION_SHOW"; then
  echo "ERROR: $SESSION_SHOW no longer invokes lookup_paired_block (FT-025)"
  FAILED=1
fi

# The CLI dispatch surface must still reach session_show.
if ! grep -q "session_show" "crates/decision-cli/src/cli/session.rs"; then
  echo "ERROR: dec CLI session.rs no longer dispatches to session_show (FT-025)"
  FAILED=1
fi

# Integration test references the four assertion targets (Paired,
# verdict value, rationale, amendment guidance). Drift in the test
# itself is treated as a regression — the test is the contract.
for token in \
  "Paired:" \
  "Verdict:" \
  "Rationale" \
  "Amendment guidance" \
  "violates"
do
  if ! grep -q -- "$token" "$TEST_FILE"; then
    echo "ERROR: $TEST_FILE no longer asserts \"$token\" (FT-025 / TC-031)"
    FAILED=1
  fi
done

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

# Behavioural invariant: the integration test exercises the rendered
# output against a synthetic fixture and asserts every required block.
# We run cargo test for that single binary so feedback is fast and the
# failure is anchored to TC-031 rather than the whole workspace.
if ! cargo test --quiet \
        --manifest-path crates/decision-cli/Cargo.toml \
        --test ft_025_session_show_paired \
        >/dev/null 2>&1; then
  echo "ERROR: ft_025_session_show_paired integration test failed (FT-025 / TC-031)"
  echo "  Re-run for detail:"
  echo "  cargo test --manifest-path crates/decision-cli/Cargo.toml --test ft_025_session_show_paired"
  exit 1
fi

echo "OK: dec session show surfaces the FT-025 paired session + verdict block (FT-025 / TC-031)"
exit 0
