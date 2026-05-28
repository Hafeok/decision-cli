#!/usr/bin/env bash
# scripts/checks/run-all-fitness.sh
#
# FT-014 / TC-086 — runs every code-structure fitness function from
# ADR-013 in one shot. The script exits non-zero on the first hard
# failure; warnings (exit 2) are surfaced but do not block the chain.
#
# The order mirrors TC-086's `Then` block:
#   1. file-length.sh
#   2. function-length.sh  (Rust)
#   3. function-length.py  (Python)
#   4. module-structure.sh
#   5. single-responsibility.sh
#
# Exit codes:
#   0 — every check exited 0.
#   1 — at least one check exited 1 (hard violation).
#   2 — at least one check exited 2 (warning) and none exited 1.
set -uo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

WORST=0

run_check() {
  local name="$1"
  shift
  echo "==> $name"
  local rc=0
  "$@" || rc=$?
  if [ "$rc" -eq 0 ]; then
    return 0
  fi
  echo "    $name exited $rc"
  if [ "$rc" -eq 1 ]; then
    WORST=1
  elif [ "$rc" -eq 2 ] && [ "$WORST" -ne 1 ]; then
    WORST=2
  else
    # Unexpected exit code; surface as a hard failure.
    WORST=1
  fi
  return 0
}

run_check "file-length.sh"          bash    "$SCRIPT_DIR/file-length.sh"
run_check "function-length.sh"      bash    "$SCRIPT_DIR/function-length.sh"
run_check "function-length.py"      python3 "$SCRIPT_DIR/function-length.py"
run_check "module-structure.sh"     bash    "$SCRIPT_DIR/module-structure.sh"
run_check "single-responsibility.sh" bash   "$SCRIPT_DIR/single-responsibility.sh"

case "$WORST" in
  0) echo "OK: all code-structure fitness functions green" ;;
  2) echo "WARN: code-structure fitness functions have warnings (no hard failures)" ;;
  *) echo "ERROR: code-structure fitness functions failed" ;;
esac

exit "$WORST"
