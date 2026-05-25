#!/usr/bin/env bash
# TC-142 — pipeline-worker SDK SSE consumer + HTTP poster exit criterion (FT-077).
# Spec: .product/tests/TC-142-pipeline-worker-sdk-sse-consumer-and-http-poster-f.md
#
# Drives the wire-protocol pytest suite against the in-memory fake harness.
# The suite exercises:
#   1. subscribe → dispatch → claim → complete (HTTP 200)
#   2. Last-Event-ID resume after disconnect
#   3. concurrent claims resolve with exactly one winner
# Plus retry/backoff and catalog cache reuse coverage.
#
# Uses the SDK's own .venv so httpx / pytest-asyncio / pydantic are isolated
# from the system Python — same pattern code-writer / verify-graph-author
# would use if they had async-runtime test deps.
set -euo pipefail

cd "$(dirname "$0")/../.."

SDK_DIR="workers/pipeline-worker-sdk"
VENV="$SDK_DIR/.venv"

if [[ ! -x "$VENV/bin/pytest" ]]; then
    echo "TC-142: bootstrapping $VENV (httpx, pydantic, pytest-asyncio)…"
    (cd "$SDK_DIR" && uv venv >/dev/null && uv pip install -e . pytest pytest-asyncio >/dev/null)
fi

exec "$VENV/bin/pytest" \
    "$SDK_DIR/tests/test_tc_142_wire_protocol.py" \
    -v --no-header
