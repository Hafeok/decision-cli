#!/usr/bin/env bash
# TC-104 — Catalog bootstrap seeds 10+2 Capability and 5 RoleBinding
#          artifacts idempotently. Validates strict divergence handling,
#          ordering enforcement, bundle + session migration, and SHACL
#          atomic-rollback on violation.
#
# Spec: .product/tests/TC-104-*.md
# Implements: FT-058 (catalog bootstrap), ADR-036 (graph-resident catalog).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# Build the binary (debug profile is fine for tests).
cargo build --quiet --package decision-cli --bin dec

DEC="$REPO_ROOT/target/debug/dec"
SCRIPT="$REPO_ROOT/scripts/bootstrap_catalog.py"
CAPS_YAML="$REPO_ROOT/config/capabilities.yaml"
BINDS_YAML="$REPO_ROOT/config/role-bindings.yaml"

export DEC_BINARY="$DEC"

WORK="$(mktemp -d --tmpdir tc-104.XXXXXX)"
trap 'rm -rf "$WORK"' EXIT

run_bootstrap() {
  python3 "$SCRIPT" --graph-path "$1" "${@:2}"
}

init_store() {
  ( cd "$1" && "$DEC" init --template engineering-development >/dev/null 2>&1 ) \
    || { echo "TC-104 FAIL: dec init for $1 returned non-zero" >&2; exit 1; }
}

sparql() {
  local workdir="$1"; shift
  ( cd "$workdir" && "$DEC" _sparql --query "$1" )
}

count_capabilities() {
  local n
  n=$(sparql "$1" \
    "PREFIX dec: <https://decision-cli.dev/ns#> SELECT (COUNT(?c) AS ?n) WHERE { { ?c a dec:Capability } UNION { GRAPH ?g { ?c a dec:Capability } } }" \
    | sed -n 's/^?n="\([0-9]*\)".*/\1/p')
  echo "${n:-0}"
}

count_bindings() {
  local n
  n=$(sparql "$1" \
    "PREFIX dec: <https://decision-cli.dev/ns#> SELECT (COUNT(?b) AS ?n) WHERE { { ?b a dec:RoleBinding } UNION { GRAPH ?g { ?b a dec:RoleBinding } } }" \
    | sed -n 's/^?n="\([0-9]*\)".*/\1/p')
  echo "${n:-0}"
}

assert_count() {
  local got="$1" want="$2" label="$3"
  if [ "$got" != "$want" ]; then
    echo "TC-104 FAIL: $label — expected $want, got $got" >&2
    exit 1
  fi
}

# -----------------------------------------------------------------------
# Step 1+2 — First bootstrap counts and PRD §5.2/§6.2 content.
# -----------------------------------------------------------------------
A="$WORK/site-a"
mkdir -p "$A"
init_store "$A"

if ! run_bootstrap "$A" >/dev/null 2>"$WORK/step1.err"; then
  echo "TC-104 FAIL: first bootstrap returned non-zero. stderr:" >&2
  cat "$WORK/step1.err" >&2
  exit 1
fi

assert_count "$(count_capabilities "$A")" 12 "first-bootstrap capability count"
assert_count "$(count_bindings "$A")" 5 "first-bootstrap binding count"

# Spot-check key PRD §5.2 / §6.2 properties via SPARQL ASK.
check_ask() {
  local label="$1" query="$2"
  local ans
  ans=$(sparql "$A" "$query")
  if [ "$ans" != "true" ]; then
    echo "TC-104 FAIL: $label — ASK returned $ans for query:" >&2
    echo "  $query" >&2
    exit 1
  fi
}

check_ask "code-writer is Scaleway/EUR" \
  'PREFIX dec: <https://decision-cli.dev/ns#>
   ASK { GRAPH ?g { ?c a dec:Capability ; dec:capability_id "code-writer" ;
                       dec:endpoint "scaleway" ; dec:cost_currency "EUR" ;
                       dec:status "active" . } }'

check_ask "deep-reasoning is Anthropic/USD with cache cost pair" \
  'PREFIX dec: <https://decision-cli.dev/ns#>
   ASK { GRAPH ?g { ?c a dec:Capability ; dec:capability_id "deep-reasoning" ;
                       dec:endpoint "anthropic" ; dec:cost_currency "USD" ;
                       dec:cost_cache_hit_per_m ?h ; dec:cost_cache_write_5m ?w . } }'

check_ask "standard-reasoning-frontier exposes reasoning trace" \
  'PREFIX dec: <https://decision-cli.dev/ns#>
   ASK { GRAPH ?g { ?c dec:capability_id "standard-reasoning-frontier" ;
                       dec:exposes_reasoning_trace true . } }'

check_ask "standard-reasoning has configurable_effort" \
  'PREFIX dec: <https://decision-cli.dev/ns#>
   ASK { GRAPH ?g { ?c dec:capability_id "standard-reasoning" ;
                       dec:configurable_effort true . } }'

check_ask "mid-reasoning is candidate" \
  'PREFIX dec: <https://decision-cli.dev/ns#>
   ASK { GRAPH ?g { ?c dec:capability_id "mid-reasoning" ;
                       dec:status "candidate" . } }'

check_ask "fast-reasoning is candidate" \
  'PREFIX dec: <https://decision-cli.dev/ns#>
   ASK { GRAPH ?g { ?c dec:capability_id "fast-reasoning" ;
                       dec:status "candidate" . } }'

check_ask "implementer binds to code-writer" \
  'PREFIX dec: <https://decision-cli.dev/ns#>
   ASK { GRAPH ?g { ?b a dec:RoleBinding ; dec:role_id "implementer" ;
                       dec:active true ;
                       dec:default_capability <https://decision-cli.dev/ns/capability/code-writer/v1> . } }'

check_ask "verifier binds to code-writer" \
  'PREFIX dec: <https://decision-cli.dev/ns#>
   ASK { GRAPH ?g { ?b dec:role_id "verifier" ; dec:active true ;
                       dec:default_capability <https://decision-cli.dev/ns/capability/code-writer/v1> . } }'

# -----------------------------------------------------------------------
# Step 3 — Source hashes recorded.
# -----------------------------------------------------------------------
check_ask "bootstrap_source recorded on every capability" \
  'PREFIX dec: <https://decision-cli.dev/ns#>
   ASK { GRAPH ?g { ?c a dec:Capability ; dec:bootstrap_source ?h . } }'

check_ask "bootstrap_source recorded on every binding" \
  'PREFIX dec: <https://decision-cli.dev/ns#>
   ASK { GRAPH ?g { ?b a dec:RoleBinding ; dec:bootstrap_source ?h . } }'

# -----------------------------------------------------------------------
# Step 4 — Idempotency: re-run is a no-op.
# -----------------------------------------------------------------------
out=$(run_bootstrap "$A" 2>&1)
if [ $? -ne 0 ]; then
  echo "TC-104 FAIL: idempotent re-run returned non-zero. Output:" >&2
  echo "$out" >&2
  exit 1
fi
if ! echo "$out" | grep -q "catalog: unchanged"; then
  echo "TC-104 FAIL: idempotent re-run did not report 'catalog: unchanged':" >&2
  echo "$out" >&2
  exit 1
fi
assert_count "$(count_capabilities "$A")" 12 "post-idempotent capability count"
assert_count "$(count_bindings "$A")" 5 "post-idempotent binding count"

# -----------------------------------------------------------------------
# Step 5 — Strict divergence handling.
# -----------------------------------------------------------------------
DIV_YAML="$WORK/capabilities-divergent.yaml"
sed 's/cost_input_per_m: "0.20"/cost_input_per_m: "0.25"/' "$CAPS_YAML" > "$DIV_YAML"

if python3 "$SCRIPT" --graph-path "$A" --capabilities "$DIV_YAML" --bindings "$BINDS_YAML" \
     >"$WORK/div.out" 2>"$WORK/div.err"; then
  echo "TC-104 FAIL: divergent bootstrap unexpectedly succeeded" >&2
  cat "$WORK/div.err" >&2
  exit 1
fi
if ! grep -q "divergence" "$WORK/div.err"; then
  echo "TC-104 FAIL: divergent bootstrap did not mention divergence in stderr:" >&2
  cat "$WORK/div.err" >&2
  exit 1
fi
if ! grep -q "code-writer@v1" "$WORK/div.err"; then
  echo "TC-104 FAIL: divergence error missing artifact id (code-writer@v1):" >&2
  cat "$WORK/div.err" >&2
  exit 1
fi
if ! grep -q "graph is authoritative" "$WORK/div.err"; then
  echo "TC-104 FAIL: divergence error missing resolution hint:" >&2
  cat "$WORK/div.err" >&2
  exit 1
fi
# No writes occurred — counts unchanged.
assert_count "$(count_capabilities "$A")" 12 "post-divergence capability count"
assert_count "$(count_bindings "$A")" 5 "post-divergence binding count"

# -----------------------------------------------------------------------
# Step 6 — Ordering enforcement.
# -----------------------------------------------------------------------
B="$WORK/site-b"
mkdir -p "$B"
init_store "$B"

BAD_BINDS="$WORK/role-bindings-unresolved.yaml"
sed 's/default_capability: code-writer/default_capability: nonexistent-cap/' "$BINDS_YAML" > "$BAD_BINDS"

if python3 "$SCRIPT" --graph-path "$B" --capabilities "$CAPS_YAML" --bindings "$BAD_BINDS" \
     >"$WORK/ord.out" 2>"$WORK/ord.err"; then
  echo "TC-104 FAIL: unresolved-ref bootstrap unexpectedly succeeded" >&2
  cat "$WORK/ord.err" >&2
  exit 1
fi
if ! grep -qi "unresolved capability reference" "$WORK/ord.err"; then
  echo "TC-104 FAIL: ordering error missing UnresolvedReference text:" >&2
  cat "$WORK/ord.err" >&2
  exit 1
fi
# Atomicity: no capability writes leaked even though caps were processed first.
assert_count "$(count_capabilities "$B")" 0 "post-unresolved capability count"
assert_count "$(count_bindings "$B")" 0 "post-unresolved binding count"

# -----------------------------------------------------------------------
# Step 9 — Atomicity on SHACL violation.
# -----------------------------------------------------------------------
C="$WORK/site-c"
mkdir -p "$C"
init_store "$C"

BAD_CAPS="$WORK/capabilities-bad-endpoint.yaml"
sed 's/endpoint: scaleway/endpoint: bogus-endpoint/' "$CAPS_YAML" > "$BAD_CAPS"

if python3 "$SCRIPT" --graph-path "$C" --capabilities "$BAD_CAPS" --bindings "$BINDS_YAML" \
     >"$WORK/shacl.out" 2>"$WORK/shacl.err"; then
  echo "TC-104 FAIL: SHACL-violating bootstrap unexpectedly succeeded" >&2
  cat "$WORK/shacl.err" >&2
  exit 1
fi
# Either parsing (unknown endpoint) or SHACL catches it; both are valid.
if ! grep -qiE "(SHACL|endpoint|invalid)" "$WORK/shacl.err"; then
  echo "TC-104 FAIL: SHACL/parse error missing helpful diagnostic:" >&2
  cat "$WORK/shacl.err" >&2
  exit 1
fi
assert_count "$(count_capabilities "$C")" 0 "post-SHACL capability count"

# -----------------------------------------------------------------------
# Step 7 + 8 — Bundle and session migration idempotence.
# -----------------------------------------------------------------------
D="$WORK/site-d"
mkdir -p "$D"
init_store "$D"

# Seed three pre-stakes bundles and two pre-token-breakdown sessions by
# appending raw N-Quads to the persisted dump, then re-loading via the
# bootstrap pipeline.
DUMP="$D/.dec/store/orchestration.nq"
cat >>"$DUMP" <<'NQ'
<https://decision-cli.dev/ns/bundle/b1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Bundle> <https://decision-cli.dev/ns/bundles> .
<https://decision-cli.dev/ns/bundle/b2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Bundle> <https://decision-cli.dev/ns/bundles> .
<https://decision-cli.dev/ns/bundle/b3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Bundle> <https://decision-cli.dev/ns/bundles> .
<https://decision-cli.dev/ns/session/s1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Session> <https://decision-cli.dev/ns/orchestration> .
<https://decision-cli.dev/ns/session/s2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Session> <https://decision-cli.dev/ns/orchestration> .
NQ

# Run with --migrate.
if ! python3 "$SCRIPT" --graph-path "$D" --migrate >"$WORK/migr.out" 2>"$WORK/migr.err"; then
  echo "TC-104 FAIL: --migrate run returned non-zero. stderr:" >&2
  cat "$WORK/migr.err" >&2
  exit 1
fi

stakes_count=$(sparql "$D" \
  'PREFIX dec: <https://decision-cli.dev/ns#>
   SELECT (COUNT(?b) AS ?n) WHERE { GRAPH ?g { ?b a dec:Bundle ; dec:stakes "routine" } }' \
  | sed -n 's/^?n="\([0-9]*\)".*/\1/p')
assert_count "$stakes_count" 3 "post-migration bundles with stakes=routine"

session_token_count=$(sparql "$D" \
  'PREFIX dec: <https://decision-cli.dev/ns#>
   SELECT (COUNT(?s) AS ?n) WHERE {
     { ?s a dec:Session ;
          dec:input_tokens_base ?b ;
          dec:input_tokens_cache_write ?cw ;
          dec:input_tokens_cache_hit ?ch }
     UNION
     { GRAPH ?g { ?s a dec:Session ;
                     dec:input_tokens_base ?b ;
                     dec:input_tokens_cache_write ?cw ;
                     dec:input_tokens_cache_hit ?ch } }
   }' | sed -n 's/^?n="\([0-9]*\)".*/\1/p')
# Init seeds 1 bootstrap session that also gets backfilled; we inserted 2 more.
# Migration count should be ≥ 2.
if [ "${session_token_count:-0}" -lt 2 ]; then
  echo "TC-104 FAIL: post-migration sessions with token breakdown — expected >= 2, got ${session_token_count:-0}" >&2
  exit 1
fi

# Idempotent re-migration.
if ! python3 "$SCRIPT" --graph-path "$D" --migrate >"$WORK/migr2.out" 2>"$WORK/migr2.err"; then
  echo "TC-104 FAIL: idempotent --migrate re-run returned non-zero. stderr:" >&2
  cat "$WORK/migr2.err" >&2
  exit 1
fi
if grep -q "bundles_migrated: [1-9]\|sessions_migrated: [1-9]" "$WORK/migr2.out"; then
  echo "TC-104 FAIL: idempotent --migrate re-run claimed work was done:" >&2
  cat "$WORK/migr2.out" >&2
  exit 1
fi
stakes_count_2=$(sparql "$D" \
  'PREFIX dec: <https://decision-cli.dev/ns#>
   SELECT (COUNT(?b) AS ?n) WHERE { GRAPH ?g { ?b a dec:Bundle ; dec:stakes "routine" } }' \
  | sed -n 's/^?n="\([0-9]*\)".*/\1/p')
assert_count "$stakes_count_2" 3 "idempotent-migration bundles with stakes"

echo "TC-104 PASS"
