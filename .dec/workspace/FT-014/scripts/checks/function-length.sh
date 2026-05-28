#!/usr/bin/env bash
# Rust function length enforcement for ADR-013 Rule 2.
# Exits 0 if all functions under crates/*/src/ are within limits.
# Exits 1 if any function exceeds FN_LENGTH_HARD (default 40 statement lines).
# Functions in the FN_LENGTH_WARN–FN_LENGTH_HARD band (default 30–40) produce
# advisory WARNING diagnostics on stdout but do not change exit code.

set -euo pipefail

# Configurable thresholds
FN_LENGTH_HARD="${FN_LENGTH_HARD:-40}"
FN_LENGTH_WARN="${FN_LENGTH_WARN:-30}"

# Track exit code: 0=clean, 1=hard violations
exit_code=0

# Find all first-party Rust source files using process substitution
# Exclude: tests.rs (inline test modules), tests/, benches/
while IFS= read -r file; do
  # Use awk to track brace depth and count statement lines per function
  awk -v file="$file" \
      -v warn_limit="$FN_LENGTH_WARN" \
      -v hard_limit="$FN_LENGTH_HARD" '
    BEGIN {
      in_fn = 0
      fn_name = ""
      fn_line = 0
      brace_depth = 0
      base_depth = 0
      stmt_count = 0
      in_cfg_test = 0
    }

    # Track #[cfg(test)] modules
    /^[[:space:]]*#\[cfg\(test\)\]/ {
      in_cfg_test = 1
      next
    }

    # Detect function declarations
    /^[[:space:]]*(pub[[:space:]]+)?(async[[:space:]]+)?fn[[:space:]]+[a-zA-Z_][a-zA-Z0-9_]*/ {
      if (!in_cfg_test && !in_fn) {
        match($0, /fn[[:space:]]+([a-zA-Z_][a-zA-Z0-9_]*)/, arr)
        fn_name = arr[1]
        fn_line = NR
        in_fn = 1
        stmt_count = 0
        base_depth = -1  # Will be set when we see the opening brace
      }
      next
    }

    {
      if (in_fn) {
        # Count braces to track depth
        for (i = 1; i <= length($0); i++) {
          c = substr($0, i, 1)
          if (c == "{") {
            brace_depth++
            if (base_depth == -1) {
              base_depth = brace_depth
            }
          } else if (c == "}") {
            brace_depth--
            if (brace_depth < base_depth) {
              # Function ended
              if (stmt_count > hard_limit) {
                printf "ERROR: %s:%d: function %s has %d statement lines (limit %d)\n", \
                  file, fn_line, fn_name, stmt_count, hard_limit
                exit 1
              } else if (stmt_count > warn_limit) {
                printf "WARNING: %s:%d: function %s has %d statement lines (warn threshold %d)\n", \
                  file, fn_line, fn_name, stmt_count, warn_limit
              }
              in_fn = 0
              fn_name = ""
              fn_line = 0
              base_depth = 0
              stmt_count = 0
              in_cfg_test = 0
              break
            }
          }
        }

        # Count non-empty, non-comment lines as statements
        if (in_fn && base_depth > 0 && brace_depth >= base_depth) {
          line = $0
          gsub(/^[[:space:]]+/, "", line)  # Strip leading whitespace
          gsub(/[[:space:]]+$/, "", line)  # Strip trailing whitespace
          # Skip empty lines and comment-only lines
          if (length(line) > 0 && line !~ /^\/\//) {
            stmt_count++
          }
        }
      }
    }
  ' "$file"

  # Capture awk exit code
  awk_exit=$?
  if [ $awk_exit -eq 1 ]; then
    exit_code=1
  fi
done < <(find crates/*/src -name '*.rs' \
  ! -name 'tests.rs' \
  ! -path '*/tests/*' \
  ! -path '*/benches/*' \
  2>/dev/null || true)

exit $exit_code
