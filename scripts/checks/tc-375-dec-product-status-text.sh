#!/usr/bin/env bash
# TC-375 / FT-145 — `dec product status` returns text summary, exit 0.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"
OUT="$(dec product status 2>&1)"
CODE=$?
if [[ "$CODE" -ne 0 ]]; then
  echo "TC-375 FAIL: dec product status exited $CODE" >&2
  echo "$OUT" >&2
  exit 1
fi
if [[ -z "$OUT" ]]; then
  echo "TC-375 FAIL: dec product status produced no output" >&2
  exit 1
fi
# Should look like a project summary — expect at least some recognisable
# substring (a phase / status / count marker).
if ! grep -qiE "phase|features|status|complete|planned" <<<"$OUT"; then
  echo "TC-375 FAIL: stdout lacks recognisable summary keywords; got:" >&2
  echo "$OUT" >&2
  exit 1
fi
echo "TC-375 PASS"
