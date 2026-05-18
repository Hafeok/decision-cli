#!/usr/bin/env bash
# TC-002 — dec init --from <path>.ttl produces an equivalent orchestration
#         store and records the source's content hash and file path on the
#         bootstrap session's PROV-O record (ADR-004, ADR-006).
#
# Spec: .product/tests/TC-002-*.md
# Implements: FT-008 (init validation), FT-009 (orchestration store).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# Build the binary (debug profile is fine for tests).
cargo build --quiet --package decision-cli --bin dec

DEC="$REPO_ROOT/target/debug/dec"
WORKDIR="$(mktemp -d --tmpdir tc-002.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

cd "$WORKDIR"

# Author a valid stream definition referencing the bundled
# `va:shipped-feature` ValueAction with the §3.2 example's authorized
# goals `(ship land)`.
mkdir -p streams
DEF_REL="./streams/decision-cli-development.ttl"
cat > "$DEF_REL" <<'EOF'
@prefix dec: <https://decision-cli.dev/ns#> .
@prefix va:  <https://decision-cli.dev/ns/value-actions/> .

<stream:decision-cli-development> a dec:ValueStream ;
    dec:name                "decision-cli-development" ;
    dec:title               "decision-cli Development" ;
    dec:description         "Value stream for shipping decision-cli features." ;
    dec:terminalValueAction va:shipped-feature ;
    dec:authorizedGoals     "ship" , "land" .
EOF

# Compute the expected SHA-256 of the source bytes directly off disk.
EXPECTED_HASH="$(sha256sum "$DEF_REL" | awk '{print $1}')"

# --- 1. dec init --from must exit 0 -----------------------------------------
if ! "$DEC" init --from "$DEF_REL"; then
  echo "TC-002 FAIL: dec init --from exited non-zero" >&2
  exit 1
fi

# --- 2. .dec/store/ exists and matches the TC-001 ValueStream/ValueAction
#       shape (one row, one terminal action). The stream IRI differs from
#       the bundled template but the cardinality is identical.
if [ ! -d ".dec/store" ]; then
  echo "TC-002 FAIL: .dec/store/ was not created" >&2
  exit 1
fi
SPARQL_BASIC='PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?stream ?action WHERE {
  ?stream a dec:ValueStream ;
          dec:terminalValueAction ?action .
  ?action a dec:ValueAction .
}'
rows=$("$DEC" _sparql --query "$SPARQL_BASIC")
row_count=$(printf '%s\n' "$rows" | grep -c .)
if [ "$row_count" -ne 1 ]; then
  echo "TC-002 FAIL: expected exactly 1 ValueStream/ValueAction row, got $row_count:" >&2
  echo "$rows" >&2
  exit 1
fi
case "$rows" in
  *"<https://decision-cli.dev/ns/value-actions/shipped-feature>"*) : ;;
  *)
    echo "TC-002 FAIL: terminal ValueAction is not <va:shipped-feature>:" >&2
    echo "$rows" >&2
    exit 1 ;;
esac

# --- 3a. Bootstrap session records the file path via prov:wasDerivedFrom. --
SPARQL_DERIVED='PREFIX dec:  <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
SELECT ?path WHERE {
  <https://decision-cli.dev/ns/session/init-001> prov:wasDerivedFrom ?path .
}'
derived_rows=$("$DEC" _sparql --query "$SPARQL_DERIVED")
case "$derived_rows" in
  *"./streams/decision-cli-development.ttl"*) : ;;
  *)
    echo "TC-002 FAIL: bootstrap session prov:wasDerivedFrom did not record the source path:" >&2
    echo "$derived_rows" >&2
    exit 1 ;;
esac

# --- 3b. The SHA-256 content hash is persisted as a literal on the session. --
SPARQL_HASH='PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?h WHERE {
  <https://decision-cli.dev/ns/session/init-001> dec:definitionHash ?h .
}'
hash_rows=$("$DEC" _sparql --query "$SPARQL_HASH")
RECORDED_HASH="$(printf '%s\n' "$hash_rows" | sed -n 's/.*"\([0-9a-f]\{64\}\)".*/\1/p' | head -n1)"
if [ -z "$RECORDED_HASH" ]; then
  echo "TC-002 FAIL: no SHA-256 hash literal on the bootstrap session:" >&2
  echo "$hash_rows" >&2
  exit 1
fi

# --- 3c. The base ontology version in effect at init time is persisted. ----
SPARQL_VERSION='PREFIX dec: <https://decision-cli.dev/ns#>
SELECT ?v WHERE {
  <https://decision-cli.dev/ns/session/init-001> dec:ontologyVersion ?v .
}'
version_rows=$("$DEC" _sparql --query "$SPARQL_VERSION")
case "$version_rows" in
  *"\""*"\""*) : ;;
  *)
    echo "TC-002 FAIL: bootstrap session did not record dec:ontologyVersion:" >&2
    echo "$version_rows" >&2
    exit 1 ;;
esac

# --- 4. The recorded hash matches the hash computed off the file on disk. --
if [ "$RECORDED_HASH" != "$EXPECTED_HASH" ]; then
  echo "TC-002 FAIL: recorded session hash $RECORDED_HASH != on-disk sha256 $EXPECTED_HASH" >&2
  exit 1
fi

# Bonus: the bootstrap session also generates the ValueStream (TC-015's
# tighter invariant), so the ASK in TC-001 must hold here too.
SPARQL_PROV='PREFIX dec: <https://decision-cli.dev/ns#>
PREFIX prov: <http://www.w3.org/ns/prov#>
ASK {
  <https://decision-cli.dev/ns/session/init-001> a dec:Session .
  ?stream a dec:ValueStream ;
          prov:wasGeneratedBy <https://decision-cli.dev/ns/session/init-001> .
}'
ask_out=$("$DEC" _sparql --query "$SPARQL_PROV")
if [ "$ask_out" != "true" ]; then
  echo "TC-002 FAIL: PROV-O ASK did not return true (got: $ask_out)" >&2
  exit 1
fi

echo "TC-002 PASS"
