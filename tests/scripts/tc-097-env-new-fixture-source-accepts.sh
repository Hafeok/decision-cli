#!/usr/bin/env bash
# TC-097 — `dec verify env new --fixture-source <path>` persists dec:fixtureSource
# on the env (FT-053 / ADR-032).
#
# Acceptance: the command exits 0 and the on-disk .ttl plus list/show
# projections expose the predicate.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-097.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

# Bootstrap a store and a fixture tree under the workdir.
"$DEC" init --template engineering-development >/dev/null 2>&1 || true
mkdir -p tests/fixtures/fixture-tc-097
echo "stub" > tests/fixtures/fixture-tc-097/README.md

# Create the env with --fixture-source.
"$DEC" verify env new \
  --id ENV-097-fixt \
  --type ephemeral-tempdir \
  --safety-class isolated \
  --allowed-ops shell,filesystem \
  --fixture-source tests/fixtures/fixture-tc-097 \
  >/dev/null

# Assertion 1: the .ttl carries the predicate.
ENV_FILE=".dec/verify/env/ENV-097-fixt.ttl"
if ! grep -q 'dec:fixtureSource "tests/fixtures/fixture-tc-097"' "$ENV_FILE"; then
  echo "TC-097 FAIL: $ENV_FILE missing dec:fixtureSource line" >&2
  cat "$ENV_FILE" >&2
  exit 1
fi

# Assertion 2: show --format json carries the field.
SHOW_JSON="$("$DEC" verify env show ENV-097-fixt --format json)"
if ! printf '%s' "$SHOW_JSON" | grep -q '"fixture_source": "tests/fixtures/fixture-tc-097"'; then
  echo "TC-097 FAIL: show JSON missing fixture_source field" >&2
  printf '%s\n' "$SHOW_JSON" >&2
  exit 1
fi

# Assertion 3: list --format json carries the field on the matching row.
LIST_JSON="$("$DEC" verify env list --format json)"
if ! printf '%s' "$LIST_JSON" | grep -q '"fixture_source": "tests/fixtures/fixture-tc-097"'; then
  echo "TC-097 FAIL: list JSON missing fixture_source field" >&2
  printf '%s\n' "$LIST_JSON" >&2
  exit 1
fi

echo "TC-097 PASS"
