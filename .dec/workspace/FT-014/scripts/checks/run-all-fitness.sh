#!/usr/bin/env bash
# Wrapper script that runs all code structure fitness functions.
# Exits 0 if all checks pass, exits 1 if any check fails.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Running code structure fitness checks..."
echo

# Track overall status
overall_status=0

# Run function-length.sh (Rust)
echo "==> Checking Rust function lengths..."
if bash "$SCRIPT_DIR/function-length.sh"; then
  echo "✓ Rust function lengths OK"
else
  echo "✗ Rust function length check failed"
  overall_status=1
fi
echo

# Run function-length.py (Python)
echo "==> Checking Python function lengths..."
if python3 "$SCRIPT_DIR/function-length.py"; then
  echo "✓ Python function lengths OK"
else
  echo "✗ Python function length check failed"
  overall_status=1
fi
echo

# Run module-structure.sh
echo "==> Checking module structure..."
if bash "$SCRIPT_DIR/module-structure.sh"; then
  echo "✓ Module structure OK"
else
  echo "✗ Module structure check failed"
  overall_status=1
fi
echo

# Run single-responsibility.sh
echo "==> Checking single-responsibility comments..."
if bash "$SCRIPT_DIR/single-responsibility.sh"; then
  echo "✓ Single-responsibility comments OK"
else
  echo "✗ Single-responsibility comment check failed"
  overall_status=1
fi
echo

if [ $overall_status -eq 0 ]; then
  echo "All code structure fitness checks passed ✓"
else
  echo "Some code structure fitness checks failed ✗"
fi

exit $overall_status
