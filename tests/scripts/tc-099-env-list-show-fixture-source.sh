#!/usr/bin/env bash
# TC-099 — list and show emit fixture_source when set, omit when unset
# (FT-053 / ADR-032).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-099.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"
"$DEC" init --template engineering-development >/dev/null 2>&1 || true
mkdir -p tests/fixtures/fixture-tc-099
echo "stub" > tests/fixtures/fixture-tc-099/marker.txt

"$DEC" verify env new \
  --id ENV-099-fixt \
  --type ephemeral-tempdir \
  --safety-class isolated \
  --allowed-ops shell,filesystem \
  --fixture-source tests/fixtures/fixture-tc-099 \
  >/dev/null

"$DEC" verify env new \
  --id ENV-099-nofixt \
  --type ephemeral-tempdir \
  --safety-class isolated \
  --allowed-ops shell,filesystem \
  >/dev/null

# Show with fixture: JSON includes the field, text includes the row.
SHOW_FIXT_JSON="$("$DEC" verify env show ENV-099-fixt --format json)"
if ! printf '%s' "$SHOW_FIXT_JSON" | grep -q '"fixture_source": "tests/fixtures/fixture-tc-099"'; then
  echo "TC-099 FAIL: show JSON for ENV-099-fixt missing fixture_source" >&2
  printf '%s\n' "$SHOW_FIXT_JSON" >&2
  exit 1
fi
SHOW_FIXT_TEXT="$("$DEC" verify env show ENV-099-fixt --format text)"
if ! printf '%s' "$SHOW_FIXT_TEXT" | grep -q 'fixture:'; then
  echo "TC-099 FAIL: show text for ENV-099-fixt missing 'fixture:' row" >&2
  printf '%s\n' "$SHOW_FIXT_TEXT" >&2
  exit 1
fi

# Show without fixture: JSON omits the field, text has no 'fixture:' row.
SHOW_NO_JSON="$("$DEC" verify env show ENV-099-nofixt --format json)"
if printf '%s' "$SHOW_NO_JSON" | grep -q '"fixture_source"'; then
  echo "TC-099 FAIL: show JSON for ENV-099-nofixt unexpectedly includes fixture_source" >&2
  printf '%s\n' "$SHOW_NO_JSON" >&2
  exit 1
fi
SHOW_NO_TEXT="$("$DEC" verify env show ENV-099-nofixt --format text)"
if printf '%s' "$SHOW_NO_TEXT" | grep -q 'fixture:'; then
  echo "TC-099 FAIL: show text for ENV-099-nofixt unexpectedly includes 'fixture:' row" >&2
  printf '%s\n' "$SHOW_NO_TEXT" >&2
  exit 1
fi

# List JSON: row for ENV-099-fixt has fixture_source; row for ENV-099-nofixt doesn't.
LIST_JSON="$("$DEC" verify env list --format json)"
python3 - <<'PYEOF' "$LIST_JSON"
import json, sys
data = json.loads(sys.argv[1])
fixt = next((e for e in data if e["id"] == "ENV-099-fixt"), None)
nofixt = next((e for e in data if e["id"] == "ENV-099-nofixt"), None)
assert fixt is not None, "ENV-099-fixt missing from list"
assert nofixt is not None, "ENV-099-nofixt missing from list"
assert fixt.get("fixture_source") == "tests/fixtures/fixture-tc-099", \
    f"ENV-099-fixt fixture_source wrong: {fixt!r}"
assert "fixture_source" not in nofixt, \
    f"ENV-099-nofixt unexpectedly carries fixture_source: {nofixt!r}"
PYEOF

echo "TC-099 PASS"
