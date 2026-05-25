#!/usr/bin/env bash
# TC-149 — pipeline-worker SDK Production EventDriver exit criterion (FT-084).
# Spec: .product/tests/TC-149-pipeline-worker-sdk-production-eventdriver-impleme.md
#
# Drives the EventDriver pytest suite. The suite exercises the two success
# criteria FT-084 names plus the Driver-Protocol conformance contract:
#   1. End-to-end dispatch → claim → session → completion against an
#      in-memory fake harness (httpx.MockTransport — same shape as TC-142).
#   2. Transient SSE disconnect mid-stream resumes via Last-Event-ID.
#   3. Transient POST failure on completion retries with backoff and
#      eventually succeeds; permanent failure surfaces CompletionFailed /
#      CompletionRejected to the worker.
#
# Uses the SDK's own .venv so pyoxigraph / pytest-asyncio / pydantic stay
# isolated from the system Python — same pattern as TC-142 / TC-143 / TC-148.
set -euo pipefail

cd "$(dirname "$0")/../.."

SDK_DIR="workers/pipeline-worker-sdk"
VENV="$SDK_DIR/.venv"

if [[ ! -x "$VENV/bin/pytest" ]] || ! "$VENV/bin/python" -c "import pyoxigraph" >/dev/null 2>&1; then
    echo "TC-149: bootstrapping $VENV (pyoxigraph, httpx, pydantic, pytest-asyncio)…"
    (cd "$SDK_DIR" && uv venv >/dev/null && uv pip install -e . pyoxigraph pytest pytest-asyncio >/dev/null)
fi

exec "$VENV/bin/pytest" \
    "$SDK_DIR/tests/test_tc_149_event_driver.py" \
    -v --no-header
