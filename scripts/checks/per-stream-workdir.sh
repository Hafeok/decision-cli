#!/usr/bin/env bash
# scripts/checks/per-stream-workdir.sh
#
# Enforces ADR-012 — each value stream lives in its own working directory;
# `dec` discovers the active stream by reading `<workdir>/.dec/store/`.
# Mechanical check: the scope loader still reads from `.dec/store/`.
#
# Exit 0: per-stream working-directory discovery is intact.
# Exit 1: the loader no longer reads `<workdir>/.dec/store/` (regression).
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

SCOPE_MOD="crates/decision-cli/src/core/scope/mod.rs"
if [ ! -f "$SCOPE_MOD" ]; then
  echo "ERROR: expected $SCOPE_MOD (ADR-012 anchor)" >&2
  exit 1
fi

# The loader must resolve `<workdir>/.dec/...` — this is what makes the
# working directory the stream's identity (ADR-012).
if ! grep -qE 'workdir\.join\("\.dec"\)' "$SCOPE_MOD"; then
  echo "ERROR: $SCOPE_MOD no longer discovers .dec/ from the working dir (ADR-012)"
  exit 1
fi

if ! grep -q 'orchestration.nq' "$SCOPE_MOD"; then
  echo "ERROR: $SCOPE_MOD no longer reads the per-workdir orchestration store (ADR-012)"
  exit 1
fi

echo "OK: per-stream working-directory discovery is intact (ADR-012)"
exit 0
