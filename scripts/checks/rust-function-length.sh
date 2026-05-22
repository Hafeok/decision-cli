#!/usr/bin/env bash
# scripts/checks/rust-function-length.sh
#
# Thin alias around `function-length.sh` — the Rust function-length
# enforcer named alongside `python-function-length.sh` so TC-086's chain
# reads as one rule per language. See ADR-013 §Rule 2.
#
# Exit codes pass through unchanged from `function-length.sh`:
#   0 — every Rust function body within the hard limit.
#   1 — at least one function exceeds the hard limit.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
exec bash "$SCRIPT_DIR/function-length.sh" "$@"
