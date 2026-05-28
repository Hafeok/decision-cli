#!/usr/bin/env bash
# TC-016: source_file_length_within_adr_013_limits
# Enforces ADR-013 Rule 1 — File Size Limit
#
# Checks that all first-party source files are within configured limits:
# - Hard limit (default 400 lines): exit 1 if exceeded
# - Warn limit (default 300 lines): stdout warning if exceeded, exit 0

set -euo pipefail

# Configuration
FILE_LENGTH_HARD=${FILE_LENGTH_HARD:-400}
FILE_LENGTH_WARN=${FILE_LENGTH_WARN:-300}

# Track violations
hard_violations=()
warn_violations=()

# Check Rust files under crates/*/src/
if [[ -d "crates" ]]; then
    while IFS= read -r -d '' file; do
        line_count=$(wc -l < "$file" | awk '{print $1}')
        if [[ $line_count -gt $FILE_LENGTH_HARD ]]; then
            hard_violations+=("$file: $line_count lines (hard limit: $FILE_LENGTH_HARD)")
        elif [[ $line_count -gt $FILE_LENGTH_WARN ]]; then
            warn_violations+=("$file: $line_count lines (warn threshold: $FILE_LENGTH_WARN)")
        fi
    done < <(find crates/*/src -type f -name "*.rs" -print0 2>/dev/null || true)
fi

# Check Python files under workers/*/ (excluding tests/)
if [[ -d "workers" ]]; then
    while IFS= read -r -d '' file; do
        # Skip files in tests/ and __pycache__/ directories
        if [[ "$file" == *"/tests/"* ]] || [[ "$file" == *"/__pycache__/"* ]]; then
            continue
        fi

        line_count=$(wc -l < "$file" | awk '{print $1}')
        if [[ $line_count -gt $FILE_LENGTH_HARD ]]; then
            hard_violations+=("$file: $line_count lines (hard limit: $FILE_LENGTH_HARD)")
        elif [[ $line_count -gt $FILE_LENGTH_WARN ]]; then
            warn_violations+=("$file: $line_count lines (warn threshold: $FILE_LENGTH_WARN)")
        fi
    done < <(find workers -type f -name "*.py" -print0 2>/dev/null || true)
fi

# Report warnings (does not affect exit code)
if [[ ${#warn_violations[@]} -gt 0 ]]; then
    for violation in "${warn_violations[@]}"; do
        echo "WARNING: $violation"
    done
fi

# Report hard violations and fail
if [[ ${#hard_violations[@]} -gt 0 ]]; then
    echo "ERROR: The following files exceed the hard limit of $FILE_LENGTH_HARD lines:"
    for violation in "${hard_violations[@]}"; do
        echo "  $violation"
    done
    exit 1
fi

exit 0
