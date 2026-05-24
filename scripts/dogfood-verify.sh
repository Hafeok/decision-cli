#!/usr/bin/env bash
# scripts/dogfood-verify.sh — drive `dec verify graph generate` over every feature.
#
# The dogfood pass for the verify-graph-author worker (FT-048/049/050).
# For each FT-NNN reported by `product feature list`, this script asks
# `dec verify graph generate FT-NNN --environment $ENV --print-only`.
# The matcher (FT-046) short-circuits features that already have a
# covering VerificationGraph in the target env, so the LLM call only
# fires for features that genuinely need a graph proposal.
#
# Modes (pick exactly one via flag):
#   --stub          Run with VERIFY_GRAPH_AUTHOR_STUB=1 set. The worker
#                   short-circuits to a deterministic Gap proposal — no
#                   LLM call, no API key needed. Use this to validate
#                   the full pipeline (resolver → bundle → worker spawn
#                   → proposal parse → coverage report).
#   --print-only    (default) Live model call, but no persistence. Each
#                   feature gets a proposal printed; nothing lands in
#                   `.dec/verify/graph/`. Requires the verify-graph-author
#                   role binding to be in the store (FT-068 — bootstrapped
#                   automatically on a fresh `dec init`; existing trees
#                   pick it up via `python3 scripts/bootstrap_catalog.py`)
#                   plus a valid `SCW_SECRET_KEY` (Scaleway, default
#                   capability per ADR-037) or `ANTHROPIC_API_KEY` if the
#                   operator has rebound the role to an Anthropic capability.
#   --accept        Live model call, persist every proposal. Mints
#                   `VG-NNN.ttl` per feature. Use only after one
#                   --print-only pass.
#
# Flags:
#   --env ENV-NNN        Target environment (default ENV-002).
#   --feature FT-NNN     Run a single feature instead of the batch.
#   --skip-covered       Skip features whose matcher reports
#                        CompleteSingle (default behaviour; here for
#                        symmetry — disable with --no-skip-covered).
#   --log-dir PATH       Where to write per-feature stdout/stderr
#                        (default ./dogfood-logs/).
#   --jobs N             Parallelism (default 1; the worker is
#                        LLM-bound, parallelism mostly helps the matcher
#                        pass when --stub is set).
#   --dry-run            Print the command for each feature, don't run.
#
# Exit codes:
#   0  every feature reached a terminal proposal (Match / New / Gap).
#   1  at least one feature errored (worker spawn failed, model call
#      failed, capability resolution failed, etc).
#   2  prereq missing (worker not on PATH, env id unknown, etc).
#
# Convention: failures do not abort the loop. The script collects
# results, writes a summary line per feature, and exits non-zero if any
# row was an error. Inspect ./dogfood-logs/FT-NNN.stderr for details.

set -uo pipefail

# Defaults.
MODE="print-only"
ENV_ID="ENV-002"
FEATURE_FILTER=""
LOG_DIR="$(pwd)/dogfood-logs"
JOBS=1
DRY_RUN=0
SKIP_COVERED=1

usage() {
    sed -n '3,40p' "$0"
}

while (($#)); do
    case "$1" in
        --stub) MODE="stub" ;;
        --print-only) MODE="print-only" ;;
        --accept) MODE="accept" ;;
        --env) shift; ENV_ID="$1" ;;
        --feature) shift; FEATURE_FILTER="$1" ;;
        --log-dir) shift; LOG_DIR="$1" ;;
        --jobs) shift; JOBS="$1" ;;
        --dry-run) DRY_RUN=1 ;;
        --skip-covered) SKIP_COVERED=1 ;;
        --no-skip-covered) SKIP_COVERED=0 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown flag: $1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done

# Prereq: worker is on PATH (FT-067 fix).
if ! command -v verify-graph-author >/dev/null 2>&1; then
    cat >&2 <<EOF
prereq missing: verify-graph-author not on PATH.
  install with: uv tool install ./workers/verify-graph-author --with anthropic
  (Scaleway-only operators may omit --with anthropic once FT-068 lands a
   pure-Scaleway code path; today the SDK is needed as a fallback.)
EOF
    exit 2
fi

# Prereq: env exists.
if ! dec verify env show "$ENV_ID" >/dev/null 2>&1; then
    echo "prereq missing: env $ENV_ID not in catalog. List: dec verify env list" >&2
    exit 2
fi

# Prereq for live modes: an API key is available. Skipped in --dry-run
# (the operator is just sanity-checking the loop).
if [ "$MODE" != "stub" ] && [ "$DRY_RUN" -eq 0 ]; then
    if [ -z "${SCW_SECRET_KEY:-}" ] && [ -z "${ANTHROPIC_API_KEY:-}" ]; then
        cat >&2 <<'EOF'
prereq missing: no model API key.
  --print-only and --accept call a live model. Export one of:
    export SCW_SECRET_KEY=...       # Scaleway (default per ADR-037)
    export ANTHROPIC_API_KEY=...    # Anthropic (only when the operator
                                    # has rebound the role)
  To validate the pipeline without an API key, run with --stub.
EOF
        exit 2
    fi
fi

mkdir -p "$LOG_DIR"

# Enumerate features.
if [ -n "$FEATURE_FILTER" ]; then
    FEATURES=("$FEATURE_FILTER")
else
    mapfile -t FEATURES < <(
        product feature list 2>/dev/null \
        | awk 'NR>2 && $1 ~ /^FT-/ {print $1}'
    )
fi

if [ "${#FEATURES[@]}" -eq 0 ]; then
    echo "no features found (product feature list returned empty)" >&2
    exit 2
fi

echo "Dogfood pass — mode=$MODE env=$ENV_ID features=${#FEATURES[@]} jobs=$JOBS"
echo "Logs: $LOG_DIR"
echo "---"

# One-row summary per feature is written to stdout. Per-feature stderr
# / stdout streams land under $LOG_DIR/FT-NNN.{stdout,stderr}.
run_one() {
    local ft="$1"
    local stdout_file="$LOG_DIR/$ft.stdout"
    local stderr_file="$LOG_DIR/$ft.stderr"

    local cli_mode_flag
    case "$MODE" in
        stub|print-only) cli_mode_flag="--print-only" ;;
        accept) cli_mode_flag="--accept" ;;
    esac

    local cmd=(dec verify graph generate "$ft" --environment "$ENV_ID" "$cli_mode_flag")

    if [ "$DRY_RUN" -eq 1 ]; then
        printf '%-10s DRY    %s\n' "$ft" "${cmd[*]}"
        return 0
    fi

    local rc=0
    if [ "$MODE" = "stub" ]; then
        VERIFY_GRAPH_AUTHOR_STUB=1 "${cmd[@]}" >"$stdout_file" 2>"$stderr_file" || rc=$?
    else
        "${cmd[@]}" >"$stdout_file" 2>"$stderr_file" || rc=$?
    fi

    # Classify the outcome from the proposal output. Patterns mirror
    # the CLI surface text in `crates/decision-cli/src/cli/verify/graph.rs`
    # (`print_match_outcome` / `print_unpersisted_new` /
    # `print_persisted_summary` / `print_gap_outcome`).
    local label
    if [ "$rc" -ne 0 ]; then
        label="ERROR"
    elif grep -q 'already covers this feature in this environment' "$stdout_file" 2>/dev/null; then
        label="MATCH"
    elif grep -q '^Gap — ' "$stdout_file" 2>/dev/null; then
        label="GAP"
    elif grep -q -E '^(Proposed New graph|Persisted VerificationGraph)' "$stdout_file" 2>/dev/null; then
        label="NEW"
    else
        label="UNK"
    fi

    printf '%-10s %-6s rc=%d  stdout=%s\n' "$ft" "$label" "$rc" "$stdout_file"
    return "$rc"
}

# Concurrency: GNU-style `xargs -P` would also work; this loop keeps the
# script dependency-free.
ANY_FAIL=0
PIDS=()
SLOTS=()
flush_slot() {
    local idx="$1"
    local pid="${PIDS[$idx]}"
    if [ -n "$pid" ]; then
        wait "$pid" || ANY_FAIL=1
        PIDS[$idx]=""
        SLOTS[$idx]=""
    fi
}
# Pre-allocate slots.
for ((i=0; i<JOBS; i++)); do PIDS[$i]=""; SLOTS[$i]=""; done

for ft in "${FEATURES[@]}"; do
    placed=0
    while [ "$placed" -eq 0 ]; do
        for ((i=0; i<JOBS; i++)); do
            if [ -z "${PIDS[$i]}" ]; then
                run_one "$ft" &
                PIDS[$i]=$!
                SLOTS[$i]="$ft"
                placed=1
                break
            fi
        done
        if [ "$placed" -eq 0 ]; then
            # All slots busy — wait for any to finish.
            for ((i=0; i<JOBS; i++)); do
                if [ -n "${PIDS[$i]}" ] && ! kill -0 "${PIDS[$i]}" 2>/dev/null; then
                    flush_slot "$i"
                    break
                fi
            done
            sleep 0.2
        fi
    done
done

for ((i=0; i<JOBS; i++)); do flush_slot "$i"; done

echo "---"
if [ "$ANY_FAIL" -ne 0 ]; then
    echo "Done with errors. Inspect $LOG_DIR/*.stderr for details." >&2
    exit 1
fi
echo "Done. Every feature reached a terminal proposal."
exit 0
