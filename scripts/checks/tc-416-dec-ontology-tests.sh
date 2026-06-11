#!/usr/bin/env bash
# scripts/checks/tc-416-dec-ontology-tests.sh
#
# Exit criteria for FT-167 (ADR-086): the unit and round-trip tests that
# moved with the pure ontology/vocab modules pass unmodified in their new
# home, `cargo test -p dec-ontology`.
#
# Exit 0: all dec-ontology tests pass. Exit 1: crate missing or tests fail.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

if [ ! -f crates/dec-ontology/Cargo.toml ]; then
  echo "ERROR: crates/dec-ontology does not exist (FT-167 not implemented)"
  exit 1
fi

cargo test -p dec-ontology --quiet
