#!/usr/bin/env bash
# TC-378 / FT-145 — status is a registered dec product verb (not "unknown
# subcommand"). Discriminates the registration_wiring cell — proves the
# dispatcher's match arm actually exists.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"
OUT="$(dec product status 2>&1 || true)"
if grep -q "unknown subcommand 'status'" <<<"$OUT"; then
  echo "TC-378 FAIL: status not registered — dispatcher returned 'unknown subcommand'" >&2
  exit 1
fi
echo "TC-378 PASS"
