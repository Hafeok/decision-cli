#!/usr/bin/env bash
# TC-004 — dec init --from <ttl-with-unknown-URI> must fail before any
#         state is written, naming the unresolvable ValueAction URI.
#
# Spec: .product/tests/TC-004-*.md
# Implements: FT-007 (bundled library — refuses unbundled URIs in slice 1
#             per ADR-006 / ADR-007), FT-008 (init validation pipeline).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# Build the binary (debug profile is fine for tests).
cargo build --quiet --package decision-cli --bin dec

DEC="$REPO_ROOT/target/debug/dec"
WORKDIR="$(mktemp -d --tmpdir tc-004.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

# Author a Turtle definition that parses and SHACL-validates, but
# references a `dec:terminalValueAction` URI that is NOT in the
# bundled set (slice 1 supports only bundled URIs — ADR-006).
UNKNOWN_URI="https://example.org/value-actions/not-bundled"
cat > unknown-action.ttl <<EOF
@prefix dec: <https://decision-cli.dev/ns#> .

<stream:unknown-uri-test> a dec:ValueStream ;
    dec:name                "unknown-uri-test" ;
    dec:title               "Unknown URI Test" ;
    dec:terminalValueAction <${UNKNOWN_URI}> ;
    dec:authorizedGoals     "ship" , "land" .
EOF

# --- 1. dec init must exit non-zero -----------------------------------------
set +e
stderr_out=$("$DEC" init --from ./unknown-action.ttl 2>&1 >/dev/null)
rc=$?
set -e
if [ "$rc" -eq 0 ]; then
  echo "TC-004 FAIL: dec init unexpectedly exited 0" >&2
  exit 1
fi

# --- 2. stderr names the unresolvable URI -----------------------------------
case "$stderr_out" in
  *"${UNKNOWN_URI}"*) : ;;
  *)
    echo "TC-004 FAIL: stderr did not name the unresolvable URI <${UNKNOWN_URI}>:" >&2
    echo "$stderr_out" >&2
    exit 1 ;;
esac

# --- 3. stderr explains slice 1 supports only bundled URIs ------------------
# Either the explicit "not bundled" phrasing or a reference to the slice 1
# bound (§6.2 / ADR-006) is acceptable — both are produced by the
# UnknownValueAction error rendering.
case "$stderr_out" in
  *"bundled"*) : ;;
  *)
    echo "TC-004 FAIL: stderr did not mention the bundled-set constraint:" >&2
    echo "$stderr_out" >&2
    exit 1 ;;
esac

# --- 4. No .dec/ directory was created --------------------------------------
if [ -d .dec ]; then
  echo "TC-004 FAIL: .dec/ was created despite the resolve-step failure" >&2
  ls -la .dec >&2
  exit 1
fi
# And no stale temp dir leaked either.
if [ -d .dec.tmp-init ]; then
  echo "TC-004 FAIL: .dec.tmp-init/ leaked after rollback" >&2
  exit 1
fi

# --- 5. Re-running with a valid input after the failure still succeeds ------
if ! "$DEC" init --template engineering-development >/dev/null 2>&1; then
  echo "TC-004 FAIL: follow-up dec init --template did not recover" >&2
  exit 1
fi
if [ ! -d .dec/store ]; then
  echo "TC-004 FAIL: follow-up init did not produce .dec/store/" >&2
  exit 1
fi

echo "TC-004 PASS"
