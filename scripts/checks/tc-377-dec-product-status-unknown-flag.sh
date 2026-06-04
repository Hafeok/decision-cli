#!/usr/bin/env bash
# TC-377 / FT-145 — unknown flag rejected with exit 2 + stderr message.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"
set +e
ERR="$(dec product status --not-a-flag 2>&1 1>/dev/null)"
CODE=$?
set -e
if [[ "$CODE" -ne 2 ]]; then
  echo "TC-377 FAIL: expected exit 2 for unknown flag, got $CODE" >&2
  echo "stderr: $ERR" >&2
  exit 1
fi
if ! grep -qE "unknown|invalid|not-a-flag" <<<"$ERR"; then
  echo "TC-377 FAIL: stderr lacks rejection message; got: $ERR" >&2
  exit 1
fi
echo "TC-377 PASS"
