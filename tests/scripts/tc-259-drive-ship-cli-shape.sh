#!/usr/bin/env bash
# TC-259 — `dec drive ship` accepts FT-XXX as first positional.
#
# Regression test: FT-113's CLI re-org left `ship` requiring a
# redundant `<GOAL>` positional, breaking `dec drive ship FT-XXX
# --bench BNCH-NNN` for every existing caller. This script
# pins the correct shape.
set -euo pipefail

# --- Assertion 1: help text shape ---
help=$(dec drive ship --help 2>&1)
if printf '%s\n' "$help" | grep -qE 'Usage: dec drive ship \[OPTIONS\] <GOAL>'; then
    printf 'REGRESSION: `dec drive ship` requires <GOAL> as first positional.\n' >&2
    printf '            Expected: `Usage: dec drive ship [OPTIONS] <ARTIFACT>` (or [ARTIFACT]).\n' >&2
    printf '            Got:\n' >&2
    printf '%s\n' "$help" | sed 's/^/    /' >&2
    exit 1
fi

# --- Assertion 2: real invocation does not error with "artifact required" ---
# FT-114 is shipped (status=complete + product verify passes), so the planner
# reaches Action::Done in iter 0 without dispatching workers. The drive
# completes in a second or two.
if ! out=$(dec drive ship FT-114 --bench BNCH-002 --max-iter 1 2>&1); then
    # Drive returned non-zero. The regression mode prints "artifact required"
    # and exits 2; surface that explicitly so the failure is unambiguous.
    if printf '%s\n' "$out" | grep -qE 'artifact required'; then
        printf 'REGRESSION: `dec drive ship FT-114 --bench BNCH-002` says "artifact required".\n' >&2
        printf '            The artifact positional must accept FT-XXX directly.\n' >&2
        printf '            Got:\n' >&2
        printf '%s\n' "$out" | sed 's/^/    /' >&2
        exit 1
    fi
    # A different failure (e.g. transient worker crash) — let the operator see
    # it but don't conflate with the regression.
    printf '`dec drive ship FT-114 --bench BNCH-002` exited non-zero unexpectedly:\n' >&2
    printf '%s\n' "$out" | sed 's/^/    /' >&2
    exit 1
fi

# Belt-and-braces: even on a zero exit, fail if the marker string appears.
if printf '%s\n' "$out" | grep -qE 'artifact required'; then
    printf 'REGRESSION: stdout contains "artifact required" even though exit was 0.\n' >&2
    printf '%s\n' "$out" | sed 's/^/    /' >&2
    exit 1
fi

exit 0
