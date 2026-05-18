#!/usr/bin/env bash
# TC-003 — dec init --from <malformed.ttl> fails before writing state, with
#         a SHACL violation message naming the missing/invalid fields.
#
# Spec: .product/tests/TC-003-*.md
# Implements: FT-006 (shapes), FT-008 (init validation).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec

DEC="$REPO_ROOT/target/debug/dec"
WORKDIR="$(mktemp -d --tmpdir tc-003.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

# Author a Turtle definition that parses cleanly but omits two
# SHACL-required fields: dec:title and dec:authorizedGoals.
cat > bad.ttl <<'EOF'
@prefix dec: <https://decision-cli.dev/ns#> .
@prefix va:  <https://decision-cli.dev/ns/value-actions/> .

<stream:bad-init> a dec:ValueStream ;
    dec:name "bad-init" ;
    dec:terminalValueAction va:shipped-feature .
EOF

# --- 1. dec init must exit non-zero -----------------------------------------
set +e
stderr_out=$("$DEC" init --from ./bad.ttl 2>&1 >/dev/null)
rc=$?
set -e
if [ "$rc" -eq 0 ]; then
  echo "TC-003 FAIL: dec init unexpectedly exited 0" >&2
  exit 1
fi

# --- 2. stderr names at least one violated SHACL shape ----------------------
case "$stderr_out" in
  *"SHACL"*"title"*) : ;;
  *)
    echo "TC-003 FAIL: stderr did not surface a SHACL violation for dec:title:" >&2
    echo "$stderr_out" >&2
    exit 1 ;;
esac
case "$stderr_out" in
  *"authorizedGoals"*) : ;;
  *)
    echo "TC-003 FAIL: stderr did not name the missing dec:authorizedGoals:" >&2
    echo "$stderr_out" >&2
    exit 1 ;;
esac
case "$stderr_out" in
  *"sh:minCount"* | *"minCount"*) : ;;
  *)
    echo "TC-003 FAIL: stderr did not cite a SHACL minCount constraint:" >&2
    echo "$stderr_out" >&2
    exit 1 ;;
esac

# --- 3. No .dec/ directory was created --------------------------------------
if [ -d .dec ]; then
  echo "TC-003 FAIL: .dec/ was created despite the failure" >&2
  ls -la .dec >&2
  exit 1
fi
# And no stale temp dir leaked either.
if [ -d .dec.tmp-init ]; then
  echo "TC-003 FAIL: .dec.tmp-init/ leaked after rollback" >&2
  exit 1
fi

# --- 4. Re-running with a valid input after the failure succeeds ------------
if ! "$DEC" init --template engineering-development >/dev/null 2>&1; then
  echo "TC-003 FAIL: follow-up dec init --template did not recover" >&2
  exit 1
fi
if [ ! -d .dec/store ]; then
  echo "TC-003 FAIL: follow-up init did not produce .dec/store/" >&2
  exit 1
fi

echo "TC-003 PASS"
