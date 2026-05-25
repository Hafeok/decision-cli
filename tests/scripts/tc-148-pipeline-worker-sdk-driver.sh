#!/usr/bin/env bash
# TC-148 — pipeline-worker SDK Driver abstraction exit criterion (FT-083).
# Spec: .product/tests/TC-148-pipeline-worker-sdk-driver-abstraction-for-product.md
#
# Drives the Driver-Protocol / FakeDriver pytest suite. The suite exercises:
#   1. The `Driver` Protocol surface is minimal (iteration + lifecycle + complete).
#   2. The `FakeDriver` accepts pre-built (bundle, expected_completion) pairs.
#   3. A worker written against `Driver` runs unchanged under the FakeDriver.
#   4. Lifecycle hooks (aclose, __aenter__/__aexit__) close cleanly.
#
# Uses the SDK's own .venv so pyoxigraph / pytest-asyncio / pydantic stay
# isolated from the system Python — same pattern as TC-142 / TC-143.
set -euo pipefail

cd "$(dirname "$0")/../.."

SDK_DIR="workers/pipeline-worker-sdk"
VENV="$SDK_DIR/.venv"

if [[ ! -x "$VENV/bin/pytest" ]] || ! "$VENV/bin/python" -c "import pyoxigraph" >/dev/null 2>&1; then
    echo "TC-148: bootstrapping $VENV (pyoxigraph, httpx, pydantic, pytest-asyncio)…"
    (cd "$SDK_DIR" && uv venv >/dev/null && uv pip install -e . pyoxigraph pytest pytest-asyncio >/dev/null)
fi

exec "$VENV/bin/pytest" \
    "$SDK_DIR/tests/test_tc_148_driver_abstraction.py" \
    -v --no-header
