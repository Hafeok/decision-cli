#!/usr/bin/env bash
# TC-007 — Unauthorized goal verb is refused with a structured message
#          naming the unauthorized goal, the stream's authorized list,
#          and the referenced ValueAction URI — before any Session,
#          Goal, or Dispatch is written (FT-010 / ADR-005).
#
# Spec: .product/tests/TC-007-*.md
# Implements: FT-010 (value stream scope enforcement; the goal
#             validation gate per ADR-005 / §3.4).
#
# Slice 1 has no `dec drive` yet (ADR-010 / ADR-011 / §6.2), so we drive
# the same code path through the hidden `dec _check-goal <goal>`
# subcommand — the underscore prefix marks it as a test/CI shim per the
# existing `_sparql` convention.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec

DEC="$REPO_ROOT/target/debug/dec"
WORKDIR="$(mktemp -d --tmpdir tc-007.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

# --- Setup: initialise the decision-cli-development stream -------------------
# Author a definition matching §3.2 (authorized goals (ship land),
# terminal ValueAction va:shipped-feature). The init pipeline records
# both authorized-goals and the terminal ValueAction onto the
# ValueStream artifact; the loader at command time reads it back.
mkdir -p streams
cat > streams/decision-cli-development.ttl <<'EOF'
@prefix dec: <https://decision-cli.dev/ns#> .
@prefix va:  <https://decision-cli.dev/ns/value-actions/> .

<stream:decision-cli-development> a dec:ValueStream ;
    dec:name                "decision-cli-development" ;
    dec:title               "decision-cli Development" ;
    dec:description         "Value stream for shipping decision-cli features." ;
    dec:terminalValueAction va:shipped-feature ;
    dec:authorizedGoals     "ship" , "land" .
EOF

if ! "$DEC" init --from ./streams/decision-cli-development.ttl >/dev/null; then
  echo "TC-007 FAIL: dec init exited non-zero" >&2
  exit 1
fi

# Snapshot the orchestration store so we can prove no Session/Goal/
# Dispatch was written by the refused goal validation.
PRE_HASH="$(sha256sum .dec/store/orchestration.nq | awk '{print $1}')"
PRE_SIZE="$(wc -c < .dec/store/orchestration.nq)"

# --- 1. Refusal: an unauthorized goal exits non-zero ------------------------
set +e
stderr_out=$("$DEC" _check-goal "prioritize" 2>&1 >/dev/null)
rc=$?
set -e
if [ "$rc" -eq 0 ]; then
  echo "TC-007 FAIL: unauthorized goal 'prioritize' was accepted (exit 0)" >&2
  echo "stderr was:" >&2
  echo "$stderr_out" >&2
  exit 1
fi

# --- 2. stderr names the unauthorized goal ----------------------------------
case "$stderr_out" in
  *"prioritize"*) : ;;
  *)
    echo "TC-007 FAIL: stderr did not name the unauthorized goal 'prioritize':" >&2
    echo "$stderr_out" >&2
    exit 1 ;;
esac

# --- 3. stderr names the stream's authorized goals --------------------------
# `ship` and `land` are both authorized; both must appear in the
# refusal so the operator can pick a compatible verb.
case "$stderr_out" in
  *"ship"*) : ;;
  *)
    echo "TC-007 FAIL: stderr did not list authorized goal 'ship':" >&2
    echo "$stderr_out" >&2
    exit 1 ;;
esac
case "$stderr_out" in
  *"land"*) : ;;
  *)
    echo "TC-007 FAIL: stderr did not list authorized goal 'land':" >&2
    echo "$stderr_out" >&2
    exit 1 ;;
esac

# --- 4. stderr names the referenced ValueAction (prefixed and/or IRI) -------
case "$stderr_out" in
  *"va:shipped-feature"*) : ;;
  *"https://decision-cli.dev/ns/value-actions/shipped-feature"*) : ;;
  *)
    echo "TC-007 FAIL: stderr did not name the referenced ValueAction:" >&2
    echo "$stderr_out" >&2
    exit 1 ;;
esac

# --- 5. Message shape matches §3.4 ------------------------------------------
# "This stream pursues `va:shipped-feature`; `prioritize` is not an
#  authorized goal — try a stream with Discovery scope."
case "$stderr_out" in
  *"This stream pursues"*) : ;;
  *)
    echo "TC-007 FAIL: stderr does not match the §3.4 message shape:" >&2
    echo "$stderr_out" >&2
    exit 1 ;;
esac

# --- 6. No Session/Goal/Dispatch was written to the orchestration store -----
# Strongest check: the persisted store bytes are byte-identical to the
# pre-refusal snapshot. (TC-014 covers the dec:inStream invariant on any
# Session/Goal/Dispatch that does get written.)
POST_HASH="$(sha256sum .dec/store/orchestration.nq | awk '{print $1}')"
POST_SIZE="$(wc -c < .dec/store/orchestration.nq)"
if [ "$PRE_HASH" != "$POST_HASH" ] || [ "$PRE_SIZE" != "$POST_SIZE" ]; then
  echo "TC-007 FAIL: orchestration store mutated despite refused goal" >&2
  echo "  pre  hash=$PRE_HASH size=$PRE_SIZE" >&2
  echo "  post hash=$POST_HASH size=$POST_SIZE" >&2
  exit 1
fi

# Belt and braces: SPARQL-level check — no dec:Session, dec:Goal, or
# dec:Dispatch artifacts in the orchestration store. (init-001 was the
# bootstrap session; we want zero *non-bootstrap* such artifacts.)
SCOPED_COUNT=$("$DEC" _sparql --query "
PREFIX dec: <https://decision-cli.dev/ns#>
SELECT (COUNT(*) AS ?n) WHERE {
  ?a a ?cls .
  VALUES ?cls { dec:Goal dec:Dispatch }
}
" 2>/dev/null | grep -oE '"[0-9]+"' | head -1 | tr -d '"')
if [ -z "$SCOPED_COUNT" ]; then SCOPED_COUNT=0; fi
if [ "$SCOPED_COUNT" != "0" ]; then
  echo "TC-007 FAIL: $SCOPED_COUNT dec:Goal/Dispatch artifacts written by refused goal" >&2
  exit 1
fi

echo "TC-007 PASS"
