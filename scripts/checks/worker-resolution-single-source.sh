#!/usr/bin/env bash
# scripts/checks/worker-resolution-single-source.sh
#
# Enforces FT-016 / TC-050: the worker resolution chain has exactly one
# definition and lives inside `crates/decision-cli/src/worker/`. Any
# duplicate `fn resolve` in the crate or any inline probe outside the
# canonical module re-introduces the gap FT-016 closed.
#
# Exit 0: invariant holds.
# Exit 1: at least one violation, offending lines on stdout.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

SRC_DIR="crates/decision-cli/src"
WORKER_DIR="$SRC_DIR/worker"

if [ ! -d "$WORKER_DIR" ]; then
  echo "ERROR: missing $WORKER_DIR (FT-016 not implemented?)"
  exit 1
fi

FAILED=0

# 1. At least one `fn resolve` lives inside the worker module.
inside_hits="$(grep -REn 'fn resolve\b' "$WORKER_DIR" 2>/dev/null || true)"
if [ -z "$inside_hits" ]; then
  echo "ERROR: no \`fn resolve\` defined inside $WORKER_DIR (TC-050 #1)"
  FAILED=1
fi

# 2. No `fn resolve` outside the worker module under crates/decision-cli/src/.
# (Some other modules may legitimately define functions named "resolve"
# unrelated to worker resolution — e.g. `resolve_workspace_dir`,
# `resolve_stream_iri`. The invariant is specifically about a function
# literally named "resolve". We grep for the strict signature.)
outside_hits="$(grep -REn 'fn resolve\b' "$SRC_DIR" 2>/dev/null \
  | grep -v "^$WORKER_DIR" || true)"
# Filter: keep only matches whose function name is exactly "resolve",
# not a longer identifier like "resolve_stream_iri".
strict_outside="$(printf '%s\n' "$outside_hits" \
  | grep -E 'fn resolve\s*[<\(]' || true)"
if [ -n "$strict_outside" ]; then
  echo "ERROR: duplicate \`fn resolve\` outside $WORKER_DIR (TC-050 #1):"
  printf '%s\n' "$strict_outside" | sed 's/^/  /'
  FAILED=1
fi

# 3. No inline `CODE_WRITER_CMD` env read outside the worker module.
env_hits="$(grep -REn '"CODE_WRITER_CMD"' "$SRC_DIR" 2>/dev/null \
  | grep -v "^$WORKER_DIR" || true)"
if [ -n "$env_hits" ]; then
  echo "ERROR: inline CODE_WRITER_CMD probe outside $WORKER_DIR (TC-050 #2):"
  printf '%s\n' "$env_hits" | sed 's/^/  /'
  FAILED=1
fi

# 4. No inline `python3 -c "import code_writer.main"` probe outside the worker
#    module.
py_hits="$(grep -REn 'python3 -c "import code_writer' "$SRC_DIR" 2>/dev/null \
  | grep -v "^$WORKER_DIR" || true)"
if [ -n "$py_hits" ]; then
  echo "ERROR: inline python3 import probe outside $WORKER_DIR (TC-050 #4):"
  printf '%s\n' "$py_hits" | sed 's/^/  /'
  FAILED=1
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi

echo "OK: worker resolution chain has a single shared implementation"
exit 0
