#!/usr/bin/env bash
# scripts/checks/graph-as-state.sh
#
# Enforces ADR-002 — orchestration state is reconstructable from the
# graph dump alone; we don't maintain a parallel event log that needs
# replay to derive current state.
#
# Exit 0: the orchestration store is serialised as an N-Quads graph dump
#         (.dec/store/orchestration.nq is the persistence path).
# Exit 1: the persistence layer no longer writes an N-Quads dump (a
#         regression away from graph-as-state).
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

PERSIST="crates/decision-cli/src/init/persist.rs"
if [ ! -f "$PERSIST" ]; then
  echo "ERROR: expected $PERSIST (ADR-002 anchor file)" >&2
  exit 1
fi

if ! grep -q "orchestration.nq" "$PERSIST"; then
  echo "ERROR: $PERSIST no longer writes orchestration.nq (ADR-002 regression)"
  exit 1
fi

if ! grep -q "RdfFormat::NQuads" "$PERSIST"; then
  echo "ERROR: $PERSIST no longer serialises via RdfFormat::NQuads (ADR-002 regression)"
  exit 1
fi

echo "OK: orchestration store persists as an N-Quads graph dump (ADR-002)"
exit 0
