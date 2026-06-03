#!/usr/bin/env bash
# TC-332-339: Test dec product <verb> forwards to product_core

set -euo pipefail

REPO_ROOT="${DEC_PROJECT_ROOT:-.}"

# Usage: dec-product-verb.sh <verb> [args...]
# Example: dec-product-verb.sh feature list
# Example: dec-product-verb.sh adr show ADR-009

cd "$REPO_ROOT"

# Run the command
dec product "$@"
exit_code=$?

# TC success criteria: command exits with 0 and produces output
if [ $exit_code -ne 0 ]; then
    echo "FAIL: dec product $* exited with code $exit_code"
    exit 1
fi

echo "PASS: dec product $* succeeded"
exit 0
