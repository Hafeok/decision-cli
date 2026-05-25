#!/usr/bin/env bash
# TC-144 — pipeline-worker SDK curated bundle facade (FT-079).
# Spec: .product/tests/TC-144-pipeline-worker-sdk-curated-query-helpers-over-the-in-memory-bundle-sub-graph-exit-criterion.md
#
# Drives the Bundle-facade pytest suite against the in-memory pyoxigraph
# store. The suite exercises the three success criteria FT-079 names:
#   1. bundle.focal() returns a typed accessor matching the bundle SHACL.
#   2. Two workers reading the same store return byte-identical accessors
#      for every curated method (determinism / idempotence).
#   3. bundle.raw_store accesses bump a session-level counter that
#      surfaces on the completion event as ``bundle_raw_store_access_count``.
#
# Uses the SDK's own .venv so pyoxigraph stays isolated from the system
# Python — same pattern as TC-142 / TC-143.
set -euo pipefail

cd "$(dirname "$0")/../.."

SDK_DIR="workers/pipeline-worker-sdk"
VENV="$SDK_DIR/.venv"

if [[ ! -x "$VENV/bin/pytest" ]] || ! "$VENV/bin/python" -c "import pyoxigraph" >/dev/null 2>&1; then
    echo "TC-144: bootstrapping $VENV (pyoxigraph, httpx, pydantic, pytest-asyncio)…"
    (cd "$SDK_DIR" && uv venv >/dev/null && uv pip install -e . pyoxigraph pytest pytest-asyncio >/dev/null)
fi

exec "$VENV/bin/pytest" \
    "$SDK_DIR/tests/test_tc_144_bundle_layer.py" \
    -v --no-header
