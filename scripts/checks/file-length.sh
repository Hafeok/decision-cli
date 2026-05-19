#!/usr/bin/env bash
# scripts/checks/file-length.sh
#
# Enforces ADR-013 Rule 1 — File Size Limit.
# Scans first-party source files under crates/*/src/ (Rust) and workers/*/
# (Python, excluding tests/) and asserts no file exceeds the hard limit.
#
# Exit 0: every first-party source file is within the warning threshold.
# Exit 1: at least one file exceeds the hard limit (default: 400 lines).
# Exit 2: at least one file is in the warning band (default: 300..=400), no
#         file exceeds the hard limit.
#
# Thresholds may be overridden via environment variables:
#   FILE_LENGTH_HARD (default: 400)
#   FILE_LENGTH_WARN (default: 300)
#
# Diagnostic output goes to stdout so `product verify` captures it in the
# TC failure record. Script-self errors go to stderr.
set -euo pipefail

HARD_LIMIT=${FILE_LENGTH_HARD:-400}
WARN_LIMIT=${FILE_LENGTH_WARN:-300}

# Resolve repository root: prefer git, fall back to PWD.
if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

# Rust under crates/<crate>/src ; Python under workers/<worker>/
# (excluding tests/ and __pycache__/).
FILES="$( {
    find crates -path '*/src/*.rs' 2>/dev/null || true
    find workers -name '*.py' \
      -not -path '*/tests/*' \
      -not -path '*/__pycache__/*' \
      -not -path '*/.venv/*' 2>/dev/null || true
  } | sort -u )"

if [ -z "$FILES" ]; then
  echo "OK: no first-party source files found"
  exit 0
fi

# wc on each file individually so we never get a "total" line and so the
# parsing below stays stable regardless of file count.
MEASUREMENTS="$(echo "$FILES" | xargs -r -n1 wc -l 2>/dev/null | awk '{print $1, $2}')"

HARD_VIOLATIONS="$(echo "$MEASUREMENTS" \
  | awk -v limit="$HARD_LIMIT" '$1 > limit {print $1, $2}' \
  | sort -rn || true)"

WARN_VIOLATIONS="$(echo "$MEASUREMENTS" \
  | awk -v wl="$WARN_LIMIT" -v hl="$HARD_LIMIT" \
    '$1 > wl && $1 <= hl {print $1, $2}' \
  | sort -rn || true)"

if [ -n "$HARD_VIOLATIONS" ]; then
  echo "ERROR: files exceeding hard limit ($HARD_LIMIT lines):"
  echo "$HARD_VIOLATIONS" | while read -r count file; do
    echo "  $file: $count lines (limit: $HARD_LIMIT)"
  done
  exit 1
fi

if [ -n "$WARN_VIOLATIONS" ]; then
  echo "WARNING: files approaching limit ($WARN_LIMIT-$HARD_LIMIT lines):"
  echo "$WARN_VIOLATIONS" | while read -r count file; do
    echo "  $file: $count lines (warn at: $WARN_LIMIT)"
  done
  exit 2
fi

echo "OK: all source files within limits (hard=$HARD_LIMIT, warn=$WARN_LIMIT)"
exit 0
