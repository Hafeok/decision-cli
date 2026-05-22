#!/usr/bin/env bash
# scripts/checks/python-function-length.sh
#
# Thin alias around `function-length.py` — the Python function-length
# enforcer named alongside `rust-function-length.sh` so TC-086's chain
# reads as one rule per language. See ADR-013 §Rule 2.
#
# Exit codes pass through unchanged from `function-length.py`:
#   0 — every Python function body within the hard limit.
#   1 — at least one function exceeds the hard limit.
#   127 — `python3` precondition not satisfied.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"

if ! command -v python3 >/dev/null 2>&1; then
  echo "ERROR: python3 not found on PATH (required for python function-length scan)" >&2
  exit 127
fi

exec python3 "$SCRIPT_DIR/function-length.py" "$@"
