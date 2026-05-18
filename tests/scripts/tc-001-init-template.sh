#!/usr/bin/env bash
# TC-001 — dec init --template engineering-development persists ValueStream
#         and ValueAction reachable via SPARQL and PROV-O-linked to the
#         bootstrap session.
#
# Spec: .product/tests/TC-001-*.md
# Implements: FT-006 (ontology), FT-007 (template), FT-008 (init validation),
#             FT-009 (orchestration store).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# Build the binary (debug profile is fine for tests).
cargo build --quiet --package decision-cli --bin dec

DEC="$REPO_ROOT/target/debug/dec"
WORKDIR="$(mktemp -d --tmpdir tc-001.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

# --- 1. dec init must exit 0 against the bundled template -------------------
if ! "$DEC" init --template engineering-development; then
  echo "TC-001 FAIL: dec init exited non-zero" >&2
  exit 1
fi

# --- 2. .dec/store/ exists and contains the persisted dump ------------------
if [ ! -d ".dec/store" ]; then
  echo "TC-001 FAIL: .dec/store/ was not created" >&2
  exit 1
fi
if [ ! -f ".dec/store/orchestration.nq" ]; then
  echo "TC-001 FAIL: persisted dump (.dec/store/orchestration.nq) is missing" >&2
  exit 1
fi

# --- 3. SPARQL — exactly one ValueStream → ValueAction row ------------------
SPARQL_BASIC='PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?stream ?action WHERE {
  ?stream a dec:ValueStream ;
          dec:terminalValueAction ?action .
  ?action a dec:ValueAction .
}'
rows=$("$DEC" _sparql --query "$SPARQL_BASIC")
row_count=$(printf '%s\n' "$rows" | grep -c .)
if [ "$row_count" -ne 1 ]; then
  echo "TC-001 FAIL: expected exactly 1 row, got $row_count:" >&2
  echo "$rows" >&2
  exit 1
fi
case "$rows" in
  *"<https://decision-cli.dev/ns/streams/engineering-development>"*"<https://decision-cli.dev/ns/value-actions/shipped-feature>"*)
    : ;;
  *)
    echo "TC-001 FAIL: SPARQL row did not bind the expected bundled URIs:" >&2
    echo "$rows" >&2
    exit 1 ;;
esac

# --- 4. Bootstrap session PROV-O reachability -------------------------------
SPARQL_PROV='PREFIX dec: <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
ASK {
  <https://decision-cli.dev/ns/session/init-001> a dec:Session .
  ?stream a dec:ValueStream ;
          prov:wasGeneratedBy <https://decision-cli.dev/ns/session/init-001> .
  ?action a dec:ValueAction ;
          prov:wasGeneratedBy <https://decision-cli.dev/ns/session/init-001> .
}'
ask_out=$("$DEC" _sparql --query "$SPARQL_PROV")
if [ "$ask_out" != "true" ]; then
  echo "TC-001 FAIL: PROV-O ASK did not return true (got: $ask_out)" >&2
  exit 1
fi

echo "TC-001 PASS"
