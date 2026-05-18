#!/usr/bin/env bash
# TC-005 — dec init --from <ttl-with-unauthorized-goal> must fail before
#         any state is written, naming the unauthorized goal AND the
#         referenced ValueAction's compatible-goals set.
#
# Spec: .product/tests/TC-005-*.md
# Implements: FT-007 (bundled `va:shipped-feature` carries the
#             `dec:compatibleGoals` set we cross-validate against),
#             FT-008 (init validation — cross-validate step of ADR-006).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec

DEC="$REPO_ROOT/target/debug/dec"
WORKDIR="$(mktemp -d --tmpdir tc-005.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

# Author a Turtle definition that parses, SHACL-validates, and resolves
# its terminal ValueAction against the bundled set — but declares an
# authorized goal (`prioritize`) that is NOT in `va:shipped-feature`'s
# `dec:compatibleGoals` (which per the bundled asset is `ship`, `land`).
SHIPPED_FEATURE_IRI="https://decision-cli.dev/ns/value-actions/shipped-feature"
cat > bad-goal.ttl <<EOF
@prefix dec: <https://decision-cli.dev/ns#> .
@prefix va:  <https://decision-cli.dev/ns/value-actions/> .

<stream:bad-goal-test> a dec:ValueStream ;
    dec:name                "bad-goal-test" ;
    dec:title               "Bad Goal Test" ;
    dec:terminalValueAction va:shipped-feature ;
    dec:authorizedGoals     "prioritize" .
EOF

# --- 1. dec init must exit non-zero -----------------------------------------
set +e
stderr_out=$("$DEC" init --from ./bad-goal.ttl 2>&1 >/dev/null)
rc=$?
set -e
if [ "$rc" -eq 0 ]; then
  echo "TC-005 FAIL: dec init unexpectedly exited 0" >&2
  exit 1
fi

# --- 2. stderr names the unauthorized goal ----------------------------------
case "$stderr_out" in
  *"prioritize"*) : ;;
  *)
    echo "TC-005 FAIL: stderr did not name the unauthorized goal 'prioritize':" >&2
    echo "$stderr_out" >&2
    exit 1 ;;
esac

# --- 3. stderr names the ValueAction's compatible-goals set -----------------
# va:shipped-feature lists `ship` and `land` as compatible; both should
# appear in the error so the operator knows the legal set.
case "$stderr_out" in
  *"ship"*) : ;;
  *)
    echo "TC-005 FAIL: stderr did not name compatible goal 'ship':" >&2
    echo "$stderr_out" >&2
    exit 1 ;;
esac
case "$stderr_out" in
  *"land"*) : ;;
  *)
    echo "TC-005 FAIL: stderr did not name compatible goal 'land':" >&2
    echo "$stderr_out" >&2
    exit 1 ;;
esac

# --- 4. stderr names the referenced ValueAction URI -------------------------
case "$stderr_out" in
  *"${SHIPPED_FEATURE_IRI}"*) : ;;
  *)
    echo "TC-005 FAIL: stderr did not name the referenced ValueAction URI:" >&2
    echo "$stderr_out" >&2
    exit 1 ;;
esac

# --- 5. No .dec/ directory was created --------------------------------------
if [ -d .dec ]; then
  echo "TC-005 FAIL: .dec/ was created despite the cross-validate-step failure" >&2
  ls -la .dec >&2
  exit 1
fi
if [ -d .dec.tmp-init ]; then
  echo "TC-005 FAIL: .dec.tmp-init/ leaked after rollback" >&2
  exit 1
fi

# --- 6. Re-running with a valid input after the failure still succeeds ------
if ! "$DEC" init --template engineering-development >/dev/null 2>&1; then
  echo "TC-005 FAIL: follow-up dec init --template did not recover" >&2
  exit 1
fi
if [ ! -d .dec/store ]; then
  echo "TC-005 FAIL: follow-up init did not produce .dec/store/" >&2
  exit 1
fi

echo "TC-005 PASS"
