#!/usr/bin/env bash
# TC-147 — pipeline-worker SDK side-channel: emergent judgments + feedback (FT-082).
# Spec: .product/tests/TC-147-pipeline-worker-sdk-emergent-judgments-and-feedbac.md
#
# Drives the side-channel pytest suite that exercises the three success
# criteria FT-082 names:
#   1. record_emergent_judgment(...) emits triples visible in the paired
#      interpretation session's bundle.
#   2. emit_feedback(blocking=True) ends the session with outcome=blocked
#      and ships the Feedback artifact in the completion.
#   3. emit_feedback(blocking=False) does not affect the outcome but still
#      ships the Feedback artifact alongside the main artifact.
#
# Uses the SDK's own .venv so pyoxigraph / pytest-asyncio / pydantic stay
# isolated from the system Python — same pattern as TC-142 / TC-143.
set -euo pipefail

cd "$(dirname "$0")/../.."

SDK_DIR="workers/pipeline-worker-sdk"
VENV="$SDK_DIR/.venv"

if [[ ! -x "$VENV/bin/pytest" ]] || ! "$VENV/bin/python" -c "import pyoxigraph" >/dev/null 2>&1; then
    echo "TC-147: bootstrapping $VENV (pyoxigraph, httpx, pydantic, pytest-asyncio)…"
    (cd "$SDK_DIR" && uv venv >/dev/null && uv pip install -e . pyoxigraph pytest pytest-asyncio >/dev/null)
fi

exec "$VENV/bin/pytest" \
    "$SDK_DIR/tests/test_tc_147_side_channel_emissions.py" \
    -v --no-header
