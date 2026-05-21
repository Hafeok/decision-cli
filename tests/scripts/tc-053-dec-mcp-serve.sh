#!/usr/bin/env bash
# TC-053 — `dec mcp serve` starts an MCP server over stdio, exposes the
#          in-memory tool registry via `tools/list`, gracefully shuts
#          down on EOF, and round-trips tool invocations through the
#          single-handler discipline (FT-034 / ADR-029).
#
# Spec: .product/tests/TC-053-*.md
# Implements: FT-034 (dec MCP server scaffolding).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# Build with fixture flag inert; the env var only matters at runtime.
cargo build --quiet --package decision-cli --bin dec

DEC="$REPO_ROOT/target/debug/dec"
WORKDIR="$(mktemp -d --tmpdir tc-053.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

# --- Setup: a minimal `dec init` so the server attaches to a real
#     working directory exactly as TC-053's Fixture spec requires.
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
  echo "TC-053 FAIL: $1" >&2
  if [[ -n "${STDOUT_FILE:-}" && -f "$STDOUT_FILE" ]]; then
    echo "--- stdout ---" >&2
    cat "$STDOUT_FILE" >&2
  fi
  if [[ -n "${STDERR_FILE:-}" && -f "$STDERR_FILE" ]]; then
    echo "--- stderr ---" >&2
    cat "$STDERR_FILE" >&2
  fi
  exit 1
}

# --- AC #1 + #3: startup + EOF shutdown -------------------------------------
STDOUT_FILE="$(mktemp)"
STDERR_FILE="$(mktemp)"
# Closing stdin (here-doc) should trigger graceful shutdown.
if ! "$DEC" mcp serve >"$STDOUT_FILE" 2>"$STDERR_FILE" <<EOF
EOF
then
  fail "dec mcp serve exited non-zero on EOF"
fi

# stderr must carry the `mcp server ready` tracing line.
if ! grep -q "mcp server ready" "$STDERR_FILE"; then
  fail "stderr missing 'mcp server ready' tracing line"
fi

# stdout must be empty when no MCP frames were sent (no chatter before
# the first wire message per FT-034 §Behaviour).
if [[ -s "$STDOUT_FILE" ]]; then
  fail "stdout non-empty before first MCP message"
fi

# --- AC #2: tools/list handshake --------------------------------------------
STDOUT_FILE="$(mktemp)"
STDERR_FILE="$(mktemp)"
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
} | "$DEC" mcp serve >"$STDOUT_FILE" 2>"$STDERR_FILE"

# Two responses (initialize + tools/list); notification produces none.
LINES=$(wc -l < "$STDOUT_FILE" | tr -d ' ')
if [[ "$LINES" -ne 2 ]]; then
  fail "expected 2 response lines, got $LINES"
fi

INIT_RESP=$(sed -n '1p' "$STDOUT_FILE")
LIST_RESP=$(sed -n '2p' "$STDOUT_FILE")

# Initialize must carry MCP protocol version + tools capability.
case "$INIT_RESP" in
  *'"protocolVersion":"2024-11-05"'*) : ;;
  *) fail "initialize response missing protocolVersion 2024-11-05" ;;
esac
case "$INIT_RESP" in
  *'"tools"'*) : ;;
  *) fail "initialize response missing 'tools' capability" ;;
esac

# tools/list must carry a `tools` array. FT-034 scaffolding ships no
# tools, so an empty array is the expected payload.
case "$LIST_RESP" in
  *'"tools":'*) : ;;
  *) fail "tools/list response missing 'tools' field" ;;
esac

# --- AC #4: tool invocation round-trips (via fixture) -----------------------
# The fixture handler `dec_mcp_ping` is registered only when
# DEC_MCP_TEST_FIXTURES=1 — verifying the structural property
# (MCP-routed invocation == direct handler invocation) without
# depending on a slice-2.5 tool that has not landed yet.
STDOUT_FILE="$(mktemp)"
STDERR_FILE="$(mktemp)"
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
  printf '%s\n' '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"dec_mcp_ping","arguments":{"hello":"world"}}}'
} | DEC_MCP_TEST_FIXTURES=1 "$DEC" mcp serve >"$STDOUT_FILE" 2>"$STDERR_FILE"

LIST_RESP=$(sed -n '2p' "$STDOUT_FILE")
CALL_RESP=$(sed -n '3p' "$STDOUT_FILE")

# Fixture must surface in tools/list.
case "$LIST_RESP" in
  *'"dec_mcp_ping"'*) : ;;
  *) fail "tools/list missing the registered fixture 'dec_mcp_ping'" ;;
esac

# tools/call must echo the arguments via the fixture's structuredContent.
case "$CALL_RESP" in
  *'"isError":false'*) : ;;
  *) fail "tools/call response not isError=false" ;;
esac
case "$CALL_RESP" in
  *'"hello":"world"'*) : ;;
  *) fail "tools/call structuredContent missing echoed arguments" ;;
esac

# --- Confirm graceful shutdown still leaves no orphan state -----------------
# A leftover .dec/store path is fine (we ran `dec init` above); but no
# stray temp dirs / lock files in $TMPDIR should remain from `dec mcp
# serve` itself.
LEAKS=$(find /tmp -maxdepth 1 -name 'dec-mcp-*' 2>/dev/null || true)
if [[ -n "$LEAKS" ]]; then
  fail "leftover dec-mcp-* state in /tmp: $LEAKS"
fi

echo "TC-053 PASS"
