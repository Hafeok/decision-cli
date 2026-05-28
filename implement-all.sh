#!/usr/bin/env bash
set -euo pipefail

# implement-all.sh — Iterate through all features and implement them headlessly.
# Uses `product feature next` to pick the next ready feature and
# `product implement --headless` to run each one without human interaction.
# Commits and pushes after each successful feature.

BRANCH=${BRANCH:-main}
MAX_FAILURES=${MAX_FAILURES:-3}
MAX_ITERATIONS=${MAX_ITERATIONS:-100}

# Auto-fill `runner` / `runner-args` on any TC linked to $1 that lacks them.
# Without this, `product verify` short-circuits with E022 and the agent run is
# discarded — even when the implementation itself was sound. Defaults to
# cargo-test with the test name derived from the TC's markdown filename, which
# matches the orchestrator's E022 hint and lets the agent honour the name when
# writing the test (or override via `product test runner ...`).
ensure_tc_runners() {
    local feature_id=$1
    local tcs
    tcs=$(product feature show "$feature_id" --format json 2>/dev/null \
            | jq -r '.tests[]? // empty')

    [ -z "$tcs" ] && return 0

    local autofilled=0
    while IFS= read -r tc; do
        [ -z "$tc" ] && continue
        local tc_file
        tc_file=$(ls .product/tests/${tc}-*.md 2>/dev/null | head -1)
        if [ -z "$tc_file" ] || [ ! -f "$tc_file" ]; then
            echo "  pre-flight: $tc — no markdown file under .product/tests/, skipping"
            continue
        fi
        if grep -qE '^runner:[[:space:]]+\S' "$tc_file" \
                && grep -qE '^runner-args:[[:space:]]+\S' "$tc_file"; then
            continue
        fi
        local id_num
        id_num=$(echo "$tc" | sed 's/TC-//')
        local slug
        slug=$(basename "$tc_file" .md | sed "s/^${tc}-//" | tr '-' '_')
        local args="tc_${id_num}_${slug}"
        echo "  pre-flight: $tc missing runner config — auto-setting runner=cargo-test args=$args timeout=120s"
        product test runner "$tc" --runner cargo-test --args "$args" --timeout 120s >/dev/null
        autofilled=$((autofilled + 1))
    done <<< "$tcs"

    if [ "$autofilled" -gt 0 ]; then
        echo "  pre-flight: auto-filled runner config on $autofilled TC(s)."
        echo "  pre-flight: the agent must write tests matching the configured names,"
        echo "              or run \`product test runner TC-XXX --args ...\` to rename them."
    fi
}

failures=0
iteration=0

while true; do
    iteration=$((iteration + 1))

    if [ "$iteration" -gt "$MAX_ITERATIONS" ]; then
        echo "Reached MAX_ITERATIONS ($MAX_ITERATIONS). Stopping."
        exit 1
    fi

    # Ask the knowledge graph what to implement next
    next_output=$(product feature next 2>&1) || true

    # Stop if nothing left
    if echo "$next_output" | grep -qi "all features are complete"; then
        echo "All features are complete."
        exit 0
    fi

    if echo "$next_output" | grep -qi "incomplete dependencies"; then
        echo "Remaining features have incomplete dependencies. Stopping."
        echo "  $next_output"
        exit 1
    fi

    # Extract feature ID (e.g. "FT-008" from "FT-008 — Schema Migration (phase 2, planned)")
    feature_id=$(echo "$next_output" | grep -oE 'FT-[0-9]+' | head -1)

    if [ -z "$feature_id" ]; then
        echo "Could not parse feature ID from: $next_output"
        exit 1
    fi

    # Extract title for the commit message (text between " — " and " (")
    feature_title=$(echo "$next_output" | sed 's/^[^ ]* — //;s/ (.*$//')

    echo "============================================================"
    echo "  Iteration $iteration: implementing $feature_id — $feature_title"
    echo "  $(date)"
    echo "============================================================"

    # Auto-fill missing TC runner config so `product verify` doesn't
    # short-circuit on E022 and discard an otherwise-good agent run.
    ensure_tc_runners "$feature_id"

    if product implement "$feature_id" --headless; then
        echo "$feature_id completed successfully."
        failures=0

        # Commit and push
        if [ -n "$(git status --porcelain)" ]; then
            git add -A
            git commit -m "Implement $feature_id: $feature_title

Automated headless implementation via product implement --headless."
            echo "Committed $feature_id."
        else
            echo "No changes to commit for $feature_id."
        fi
    else
        failures=$((failures + 1))
        echo "WARNING: $feature_id failed (consecutive failures: $failures/$MAX_FAILURES)"

        # Discard partial work so the next iteration starts clean
        git checkout -- . 2>/dev/null || true
        git clean -fd 2>/dev/null || true

        if [ "$failures" -ge "$MAX_FAILURES" ]; then
            echo "Hit $MAX_FAILURES consecutive failures. Stopping."
            exit 1
        fi

        echo "Skipping to next feature..."
    fi

    echo ""
done
