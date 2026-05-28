#!/usr/bin/env bash
# Single responsibility comment enforcement for ADR-013 Rule 4.
# Exits 0 if every first-party Rust file's first non-shebang line starts with `//! `
# AND every first-party Python file's first non-shebang line starts with `"""`,
# AND neither sentence contains the substring ` and `.
# Exits 1 if any file is missing the responsibility comment OR contains ` and `.

set -euo pipefail

# Track exit code
exit_code=0

# Check Rust files using process substitution to avoid subshell variable scope issues
while IFS= read -r file; do
  # Read first non-empty, non-shebang line
  first_line=$(grep -v '^[[:space:]]*$' "$file" | grep -v '^#!' | head -n 1 || true)

  # Check if it starts with //!
  if [[ ! "$first_line" =~ ^[[:space:]]*//![[:space:]] ]]; then
    echo "ERROR: $file: missing or malformed responsibility comment (expected '//! ')"
    exit_code=1
    continue
  fi

  # Check for dual responsibility ( and )
  if echo "$first_line" | grep -q ' and '; then
    echo "ERROR: $file: responsibility comment contains ' and ' (dual responsibility)"
    exit_code=1
  fi
done < <(find crates/*/src -name '*.rs' \
  ! -name 'tests.rs' \
  ! -path '*/tests/*' \
  ! -path '*/benches/*' \
  2>/dev/null || true)

# Check Python files
if [[ -d "workers" ]]; then
  while IFS= read -r file; do
    # Read first non-empty, non-shebang line
    first_line=$(grep -v '^[[:space:]]*$' "$file" | grep -v '^#!' | head -n 1 || true)

    # Check if it starts with """
    if [[ ! "$first_line" =~ ^[[:space:]]*\"\"\" ]]; then
      echo "ERROR: $file: missing or malformed responsibility comment (expected '\"\"\"')"
      exit_code=1
      continue
    fi

    # Check for dual responsibility ( and )
    if echo "$first_line" | grep -q ' and '; then
      echo "ERROR: $file: responsibility comment contains ' and ' (dual responsibility)"
      exit_code=1
    fi
  done < <(find workers -name '*.py' \
    ! -path '*/tests/*' \
    ! -path '*/__pycache__/*' \
    ! -path '*/.venv/*' \
    ! -path '*/.pytest_cache/*' \
    2>/dev/null || true)
fi

exit $exit_code
