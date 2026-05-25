#!/usr/bin/env bash
# TC-143 — pipeline-worker SDK one-dispatch-one-completion lifecycle (FT-078).
# Spec: .product/tests/TC-143-pipeline-worker-sdk-one-dispatch-to-one-completi.md
#
# Drives the Session-lifecycle pytest suite against the in-memory pyoxigraph
# store. The suite exercises the four success criteria FT-078 names:
#   1. N bundle triples in -> Session.store has exactly N quads.
#   2. Clean completion includes artifact + side-channel + telemetry.
#   3. Blocked completion preserves side-channel triples after an exception.
#   4. Session URI matches the harness's prov:Activity URI (ADR-050).
#
# Uses the SDK's own .venv so pyoxigraph / pytest-asyncio / pydantic stay
# isolated from the system Python — same pattern as TC-142.
set -euo pipefail

cd "$(dirname "$0")/../.."

SDK_DIR="workers/pipeline-worker-sdk"
VENV="$SDK_DIR/.venv"

if [[ ! -x "$VENV/bin/pytest" ]] || ! "$VENV/bin/python" -c "import pyoxigraph" >/dev/null 2>&1; then
    echo "TC-143: bootstrapping $VENV (pyoxigraph, httpx, pydantic, pytest-asyncio)…"
    (cd "$SDK_DIR" && uv venv >/dev/null && uv pip install -e . pyoxigraph pytest pytest-asyncio >/dev/null)
fi

exec "$VENV/bin/pytest" \
    "$SDK_DIR/tests/test_tc_143_session_lifecycle.py" \
    -v --no-header
