#!/usr/bin/env bash
# TC-051 — Every registered MCP tool follows `dec_<noun>_<verb>` naming
#          (FT-034 / ADR-029). Exercises:
#            • naming enforcement at registration (unit-test layer)
#            • live registry conformance (regex over `tools/list`)
#            • duplicate registration fails server startup (unit-test layer)
#
# Spec: .product/tests/TC-051-*.md
# Implements: FT-034 (dec MCP server scaffolding).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec

DEC="$REPO_ROOT/target/debug/dec"
WORKDIR="$(mktemp -d --tmpdir tc-051.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"
mkdir -p streams
cat > "./streams/decision-cli-development.ttl" <<'EOF'
@prefix dec: <https://decision-cli.dev/ns#> .
@prefix va:  <https://decision-cli.dev/ns/value-actions/> .

<stream:decision-cli-development> a dec:ValueStream ;
    dec:name                "decision-cli-development" ;
    dec:title               "decision-cli Development" ;
    dec:description         "Value stream for shipping decision-cli features." ;
    dec:terminalValueAction va:shipped-feature ;
    dec:authorizedGoals     "ship" , "land" .
EOF
"$DEC" init --from "./streams/decision-cli-development.ttl" >/dev/null || true

fail() {
  echo "TC-051 FAIL: $1" >&2
  exit 1
}

# --- AC #1: naming enforcement (the unit-tests assert specific rejection
#     variants; here we re-run them to keep this script self-contained
#     and to surface failures via the bash runner contract).
pushd "$REPO_ROOT" >/dev/null
cargo test --quiet --package decision-cli --lib \
  -- core::mcp::naming::tests \
  >/dev/null 2>&1 || fail "naming unit tests failed"
cargo test --quiet --package decision-cli --lib \
  -- core::mcp::registry::tests::rejects_malformed_name \
  >/dev/null 2>&1 || fail "registry rejects_malformed_name failed"
popd >/dev/null

# --- AC #2: live registry conformance. Every name in `tools/list` must
#     match ^dec_[a-z0-9]+(_[a-z0-9]+)*$ . Run twice: once empty (FT-034
#     ships no tools by default) and once with the fixture set on (the
#     fixture tools themselves must conform).
STDOUT_FILE="$(mktemp)"

run_list() {
  local env_prefix=$1
  : >"$STDOUT_FILE"
  {
    printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'
    printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
  } | env $env_prefix "$DEC" mcp serve >"$STDOUT_FILE" 2>/dev/null
}

assert_conformant() {
  local label=$1
  # Parse the tools/list response (line 2) and extract the names from
  # the tools array — never from initialize.serverInfo.name.
  local list_line
  list_line=$(sed -n '2p' "$STDOUT_FILE")
  local names
  names=$(printf '%s' "$list_line" | jq -r '.result.tools[].name // empty')
  if [[ -z "$names" ]]; then
    echo "  ($label) no tools — trivially conformant"
    return
  fi
  while IFS= read -r n; do
    [[ -z "$n" ]] && continue
    if ! [[ "$n" =~ ^dec_[a-z0-9]+(_[a-z0-9]+)*$ ]]; then
      fail "($label) tool name '$n' violates ^dec_[a-z0-9]+(_[a-z0-9]+)*$"
    fi
    echo "  ($label) OK: $n"
  done <<< "$names"
}

run_list ""
assert_conformant "production"

run_list "DEC_MCP_TEST_FIXTURES=1"
assert_conformant "with-fixtures"

# Sanity: the fixture run must have actually loaded `dec_mcp_ping`,
# otherwise the conformance check above is vacuous.
if ! grep -q '"dec_mcp_ping"' "$STDOUT_FILE"; then
  fail "fixture run did not register dec_mcp_ping (env propagation broken)"
fi

# --- AC #3: duplicate registration is a startup failure.
# The unit test `core::mcp::registry::tests::rejects_duplicate_name`
# asserts the registry-level rejection. We re-run it here so the
# bash runner contract catches regressions on this AC too.
pushd "$REPO_ROOT" >/dev/null
cargo test --quiet --package decision-cli --lib \
  -- core::mcp::registry::tests::rejects_duplicate_name \
  >/dev/null 2>&1 || fail "registry rejects_duplicate_name failed"
popd >/dev/null

echo "TC-051 PASS"
