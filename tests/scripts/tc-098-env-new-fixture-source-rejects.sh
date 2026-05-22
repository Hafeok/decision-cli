#!/usr/bin/env bash
# TC-098 — `dec verify env new --fixture-source <path>` rejects unsafe values
# (FT-053 / ADR-032).
#
# Acceptance: each invalid variant exits non-zero with stderr naming the
# `fixture_source` field and the failure class.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-098.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"
"$DEC" init --template engineering-development >/dev/null 2>&1 || true
mkdir -p tests
echo "stub" > Cargo.toml

run_and_expect_failure () {
  local fixture="$1"
  local needle="$2"
  local id="$3"
  local stderr_file
  stderr_file="$(mktemp)"
  set +e
  "$DEC" verify env new \
    --id "$id" \
    --type ephemeral-tempdir \
    --safety-class isolated \
    --allowed-ops shell,filesystem \
    --fixture-source "$fixture" \
    >/dev/null 2>"$stderr_file"
  local rc=$?
  set -e
  if [ "$rc" = "0" ]; then
    echo "TC-098 FAIL: variant $id with fixture=$fixture unexpectedly exited 0" >&2
    cat "$stderr_file" >&2
    rm -f "$stderr_file"
    exit 1
  fi
  if ! grep -q 'fixture_source' "$stderr_file"; then
    echo "TC-098 FAIL: variant $id stderr missing fixture_source mention" >&2
    cat "$stderr_file" >&2
    rm -f "$stderr_file"
    exit 1
  fi
  if ! grep -q "$needle" "$stderr_file"; then
    echo "TC-098 FAIL: variant $id stderr missing class hint '$needle'" >&2
    cat "$stderr_file" >&2
    rm -f "$stderr_file"
    exit 1
  fi
  rm -f "$stderr_file"
  if [ -f ".dec/verify/env/$id.ttl" ]; then
    echo "TC-098 FAIL: variant $id should not have written .ttl file" >&2
    exit 1
  fi
}

run_and_expect_failure "/etc"                               "repo-relative"    "ENV-098-abs"
run_and_expect_failure "tests/../etc"                       ".."                "ENV-098-par"
run_and_expect_failure "tests/fixtures/__does_not_exist__"  "does not exist"   "ENV-098-mis"
run_and_expect_failure "Cargo.toml"                         "not a directory"  "ENV-098-fil"

echo "TC-098 PASS"
