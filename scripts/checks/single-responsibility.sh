#!/usr/bin/env bash
# scripts/checks/single-responsibility.sh
#
# Enforces ADR-013 Rule 4 — Single Responsibility Naming Contract.
# Every first-party source file must begin with a one-sentence
# responsibility comment whose first line does NOT contain the word
# "and" (with surrounding spaces). If it does, the file has two
# responsibilities and must be split.
#
# - Rust files (`crates/*/src/**/*.rs`) must begin with `//! `.
# - Python files (`workers/*/**/*.py`, excluding tests/) must begin with
#   `"""` (a module docstring). A leading `#!` shebang is skipped before
#   the docstring requirement applies.
#
# Exit 0: every first-party source file's responsibility line is well-formed.
# Exit 1: at least one violation (missing comment or contains " and ").
# Exit 2: reserved for future warning conditions; not used today.
#
# Diagnostic output goes to stdout. Script-self errors go to stderr.
set -euo pipefail

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

RUST_FILES="$(find crates -path '*/src/*.rs' \
  -not -name 'tests.rs' \
  -not -path '*/tests/*' \
  -not -path '*/benches/*' 2>/dev/null | sort -u || true)"

PY_FILES="$(find workers -name '*.py' \
  -not -path '*/tests/*' \
  -not -path '*/__pycache__/*' \
  -not -path '*/.venv/*' 2>/dev/null | sort -u || true)"

VIOLATIONS=""

# first_non_shebang_line FILE
#   Prints the first non-shebang, non-blank line of FILE.
first_non_shebang_line() {
  awk '
    NR == 1 && /^#!/ { next }
    /^[[:space:]]*$/ { next }
    { print; exit }
  ' "$1"
}

# Rust: first line must start with `//! `.
if [ -n "${RUST_FILES// }" ]; then
  while IFS= read -r f; do
    [ -z "$f" ] && continue
    line="$(first_non_shebang_line "$f")"
    if ! printf '%s\n' "$line" | grep -qE '^//! '; then
      VIOLATIONS="${VIOLATIONS}MISSING\t$f\tRust file does not start with \"//! \" responsibility comment\n"
      continue
    fi
    # Strip the `//! ` prefix to examine the sentence proper.
    sentence="$(printf '%s' "$line" | sed 's|^//! ||')"
    if printf '%s' "$sentence" | grep -qE '(^| )and( |$)'; then
      VIOLATIONS="${VIOLATIONS}DUAL\t$f\t$line\n"
    fi
  done <<<"$RUST_FILES"
fi

# Python: first line (post-shebang) must start with `"""`.
if [ -n "${PY_FILES// }" ]; then
  while IFS= read -r f; do
    [ -z "$f" ] && continue
    line="$(first_non_shebang_line "$f")"
    if ! printf '%s\n' "$line" | grep -qE '^"""'; then
      VIOLATIONS="${VIOLATIONS}MISSING\t$f\tPython file does not start with a triple-quoted docstring\n"
      continue
    fi
    # Strip the leading `"""` and any trailing `"""`.
    sentence="$(printf '%s' "$line" | sed -E 's|^"""||; s|"""[[:space:]]*$||')"
    if printf '%s' "$sentence" | grep -qE '(^| )and( |$)'; then
      VIOLATIONS="${VIOLATIONS}DUAL\t$f\t$line\n"
    fi
  done <<<"$PY_FILES"
fi

if [ -n "${VIOLATIONS//[$' \t\n']/}" ]; then
  echo "ERROR: single-responsibility comment violations (ADR-013 Rule 4):"
  # shellcheck disable=SC2059
  printf "$VIOLATIONS" | while IFS=$'\t' read -r kind file detail; do
    [ -z "$file" ] && continue
    case "$kind" in
      MISSING) echo "  $file — $detail" ;;
      DUAL)    echo "  $file — first line contains \" and \" (two responsibilities): $detail" ;;
    esac
  done
  exit 1
fi

echo "OK: all first-party source files have a well-formed single-responsibility comment"
exit 0
