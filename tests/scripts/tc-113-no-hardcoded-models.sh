#!/usr/bin/env bash
# TC-113 — worker layer has no hardcoded model identifiers outside
# model_router and test fixtures.
#
# Spec: .product/tests/TC-113-worker-layer-has-no-hardcoded-model-identifiers-ou.md
# Validates: FT-064 migration cleanup (ADR-033, PRD §11.5).
#
# Strategy: a series of grep checks against the worker source tree,
# allowlisting only the central ModelRouter (workers/_shared/src/_shared/
# model_router.py) and test fixtures. Each check must return zero matches
# (grep exits 1) outside the allowed paths. After the structural checks,
# the existing verifier + code-writer tests are run end-to-end to assert
# the migration changed plumbing rather than behaviour (TC-113 step 6).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# ---------------------------------------------------------------------------
# Allowlists
# ---------------------------------------------------------------------------
ROUTER_PATH="workers/_shared/src/_shared/model_router.py"

# Common grep arguments for the structural checks.
GREP_COMMON=(
    -rn
    --include='*.py'
    --exclude-dir=tests
    --exclude-dir=__pycache__
    --exclude-dir=.venv
)

fail() {
    echo "TC-113 FAIL: $1" >&2
    if [ -n "${2:-}" ]; then
        echo "Offending matches:" >&2
        echo "$2" >&2
    fi
    exit 1
}

# ---------------------------------------------------------------------------
# Step 1 — No "claude-(sonnet|opus|haiku)" outside model_router.py.
# ---------------------------------------------------------------------------
MATCHES=$(grep "${GREP_COMMON[@]}" -E 'claude-(sonnet|opus|haiku)' workers/ \
    | grep -v "$ROUTER_PATH" || true)
if [ -n "$MATCHES" ]; then
    fail "hardcoded Anthropic model id found outside $ROUTER_PATH" "$MATCHES"
fi

# ---------------------------------------------------------------------------
# Step 2 — No "qwen3", "devstral", "gpt-oss", "mistral-small" outside
# model_router.py. Catches accidental Scaleway model id constants.
# ---------------------------------------------------------------------------
MATCHES=$(grep "${GREP_COMMON[@]}" -E 'qwen3|devstral|gpt-oss|mistral-small' workers/ \
    | grep -v "$ROUTER_PATH" || true)
if [ -n "$MATCHES" ]; then
    fail "hardcoded Scaleway model id found outside $ROUTER_PATH" "$MATCHES"
fi

# ---------------------------------------------------------------------------
# Step 3 — No "*MODEL_ID*" module-level constants outside model_router.py.
# Pattern matches assignments at column 0 (module-level), e.g.
# DEFAULT_MODEL_ID = "…". Matches inside functions (indented) do not
# trigger because the regex anchors at start-of-line. The router's own
# endpoint constants would be allowed via $ROUTER_PATH allowlist.
# ---------------------------------------------------------------------------
MATCHES=$(grep "${GREP_COMMON[@]}" -E '^[A-Z_]*MODEL_ID[A-Z_]*[[:space:]]*=' workers/ \
    | grep -v "$ROUTER_PATH" || true)
if [ -n "$MATCHES" ]; then
    fail "DEFAULT_MODEL_ID-shaped module constant found outside $ROUTER_PATH" "$MATCHES"
fi

# ---------------------------------------------------------------------------
# Step 4 — No "VERIFIER_MODEL_ID" env-var resolution remains anywhere
# under workers/ (per TC-113 §4: zero matches, not "outside fixtures").
# ---------------------------------------------------------------------------
MATCHES=$(grep -rn 'VERIFIER_MODEL_ID' workers/ \
    --include='*.py' --exclude-dir=__pycache__ --exclude-dir=.venv \
    || true)
if [ -n "$MATCHES" ]; then
    fail "VERIFIER_MODEL_ID env-var override still referenced under workers/" "$MATCHES"
fi

# ---------------------------------------------------------------------------
# Step 5 — anthropic.Anthropic() construction is centralised in
# model_router.py only.
# ---------------------------------------------------------------------------
MATCHES=$(grep "${GREP_COMMON[@]}" -E 'anthropic\.Anthropic\(\)' workers/ \
    | grep -v "$ROUTER_PATH" || true)
if [ -n "$MATCHES" ]; then
    fail "anthropic.Anthropic() constructed outside $ROUTER_PATH" "$MATCHES"
fi

# ---------------------------------------------------------------------------
# Step 6 — pre-migration tests still pass. The migration changes the
# plumbing (model id arrives via dispatch payload), not the behaviour
# (the same VerificationVerdict / CodeChange shapes are produced).
# ---------------------------------------------------------------------------
echo "TC-113 structural checks PASS; running worker test suites…" >&2

run_uv_pytest() {
    local worker_dir="$1"
    if [ ! -d "$worker_dir" ]; then
        echo "  skip: $worker_dir not present" >&2
        return 0
    fi
    (cd "$worker_dir" && uv sync --quiet >/dev/null 2>&1 && uv run pytest tests/ -q) \
        || fail "pytest failed under $worker_dir"
}

run_uv_pytest "workers/verifier"
run_uv_pytest "workers/code-writer"

echo "TC-113 PASS: worker layer is free of hardcoded model identifiers." >&2
