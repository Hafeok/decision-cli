#!/usr/bin/env bash
# File length enforcement for ADR-013 Rule 1.
# Counts lines in every first-party source file under crates/*/src/ and workers/*/.
# Exit 0 if clean, 1 if any file exceeds hard limit, diagnostics to stdout.

set -euo pipefail

FILE_LENGTH_HARD="${FILE_LENGTH_HARD:-500}"
FILE_LENGTH_WARN="${FILE_LENGTH_WARN:-400}"

exit_code=0

# Find all first-party source files
rust_files=$(find crates/*/src -type f -name '*.rs' 2>/dev/null | \
  grep -v '/tests/' | \
  grep -v '/benches/' || true)

py_files=$(find workers -type f -name '*.py' 2>/dev/null | \
  grep -v '/tests/' | \
  grep -v '/__pycache__/' | \
  grep -v '/.venv/' || true)

all_files="$rust_files"$'\n'"$py_files"

if [[ -z "$all_files" ]] || [[ "$all_files" == $'\n' ]]; then
  echo "OK: no first-party source files found"
  exit 0
fi

while IFS= read -r file; do
  [[ -n "$file" ]] || continue
  [[ -s "$file" ]] || continue

  line_count=$(wc -l < "$file")

  if (( line_count > FILE_LENGTH_HARD )); then
    echo "ERROR: ${file}: ${line_count} lines (limit: ${FILE_LENGTH_HARD})"
    exit_code=1
  elif (( line_count > FILE_LENGTH_WARN )); then
    echo "WARNING: ${file}: ${line_count} lines (warn threshold: ${FILE_LENGTH_WARN})"
  fi
done <<< "$all_files"

if [[ $exit_code -eq 0 ]]; then
  echo "OK: all files within length limits"
fi

exit $exit_code
