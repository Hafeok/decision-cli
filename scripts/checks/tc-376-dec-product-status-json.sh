#!/usr/bin/env bash
# TC-376 / FT-145 — `dec product status --format json` emits parseable JSON.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"
OUT="$(dec product status --format json 2>&1)"
CODE=$?
if [[ "$CODE" -ne 0 ]]; then
  echo "TC-376 FAIL: dec product status --format json exited $CODE" >&2
  echo "$OUT" >&2
  exit 1
fi
# Parse with python json.loads — fails if malformed.
if ! python3 -c "import json,sys; d=json.loads(sys.stdin.read()); assert isinstance(d, dict), 'not an object'" <<<"$OUT"; then
  echo "TC-376 FAIL: stdout is not valid JSON object" >&2
  echo "$OUT" >&2
  exit 1
fi
echo "TC-376 PASS"
