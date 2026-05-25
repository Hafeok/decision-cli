#!/usr/bin/env bash
# TC-145 — pipeline-worker SDK typed artifact builders with SHACL validation at commit (FT-080).
# Spec: .product/tests/TC-145-pipeline-worker-sdk-typed-artifact-builders-with-s.md
#
# Drives the artifact-builder pytest suite against the in-memory pyoxigraph
# store. The suite exercises the three success criteria FT-080 names:
#   1. A builder missing a required field — including a required motivational
#      predicate — raises on commit() with a SHACL-derived error message,
#      before any wire send.
#   2. Local SHACL-derived validation emits triples shaped exactly as the
#      per-type shape (FT-072) declares; harness re-validates (FT-073).
#   3. Calls to emit_triple increment a telemetry counter visible on the
#      completion event (artifact_escape_hatch_count).
#
# Uses the SDK's own .venv so pyoxigraph / pydantic / pytest stay isolated
# from the system Python — same pattern as TC-142 / TC-143 / TC-144.
set -euo pipefail

cd "$(dirname "$0")/../.."

SDK_DIR="workers/pipeline-worker-sdk"
VENV="$SDK_DIR/.venv"

if [[ ! -x "$VENV/bin/pytest" ]] || ! "$VENV/bin/python" -c "import pyoxigraph" >/dev/null 2>&1; then
    echo "TC-145: bootstrapping $VENV (pyoxigraph, httpx, pydantic, pytest-asyncio)…"
    (cd "$SDK_DIR" && uv venv >/dev/null && uv pip install -e . pyoxigraph pytest pytest-asyncio >/dev/null)
fi

exec "$VENV/bin/pytest" \
    "$SDK_DIR/tests/test_tc_145_artifact_builder_commit.py" \
    -v --no-header
