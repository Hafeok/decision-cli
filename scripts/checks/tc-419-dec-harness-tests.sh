#!/usr/bin/env bash
# scripts/checks/tc-419-dec-harness-tests.sh
#
# Exit criteria for FT-169 (ADR-086): the dispatch, drive, worker,
# subscription, and verification-orchestration tests that moved with the
# harness pass unmodified in their new home, `cargo test -p dec-harness`.
set -euo pipefail
if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi
if [ ! -f crates/dec-harness/Cargo.toml ]; then
  echo "ERROR: crates/dec-harness does not exist (FT-169 not implemented)"
  exit 1
fi
cargo test -p dec-harness --quiet
