#!/usr/bin/env bash
# TC-146 — pipeline-worker SDK LiteLLM provider with capability-tag dispatch
# and structured output (FT-081).
# Spec: .product/tests/TC-146-pipeline-worker-sdk-litellm-client-with-capability.md
#
# Drives the Provider/LiteLLMClient pytest suite. The suite exercises the
# four success criteria FT-081 names:
#   1. capability_tag -> LiteLLM model group -> Pydantic instance round-trip.
#   2. Synchronous telemetry (tokens, model, provider, latency, retries)
#      attaches to the session's completion payload.
#   3. metadata={"ddd_session_id": ...} propagates to LiteLLM for callback
#      correlation with pipeline-cli's async cost telemetry.
#   4. LITELLM_BASE_URL / LITELLM_API_KEY env vars reconfigure the endpoint
#      with no code change (ADR-053).
#
# Uses the SDK's own .venv so litellm / instructor / pyoxigraph / pytest
# stay isolated from the system Python — same pattern as TC-142 and TC-143.
set -euo pipefail

cd "$(dirname "$0")/../.."

SDK_DIR="workers/pipeline-worker-sdk"
VENV="$SDK_DIR/.venv"

needs_bootstrap=0
if [[ ! -x "$VENV/bin/pytest" ]]; then
    needs_bootstrap=1
elif ! "$VENV/bin/python" -c "import pyoxigraph, litellm, instructor" >/dev/null 2>&1; then
    needs_bootstrap=1
fi

if (( needs_bootstrap )); then
    echo "TC-146: bootstrapping $VENV (litellm, instructor, pyoxigraph, pytest)…"
    (
        cd "$SDK_DIR" \
        && uv venv >/dev/null \
        && uv pip install -e . pyoxigraph pytest pytest-asyncio litellm instructor >/dev/null
    )
fi

exec "$VENV/bin/pytest" \
    "$SDK_DIR/tests/test_tc_146_provider_capability_dispatch.py" \
    -v --no-header
