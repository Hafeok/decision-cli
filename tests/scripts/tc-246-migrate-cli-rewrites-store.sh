#!/usr/bin/env bash
# TC-246 — Hidden CLI dec migrate env-to-bench rewrites class assertion
# and predicate IRIs in a live workdir-shaped store.
#
# Spec: .product/tests/TC-246-*.md
# Implements: FT-117

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-246.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT
mkdir -p "$WORKDIR/.dec/store"

# Seed a workdir-shaped orchestration.nq with 3 quads carrying the
# legacy ENV vocabulary in a real named graph (NOT default graph,
# matching production layout where every artifact lives under a
# named graph).
cat > "$WORKDIR/.dec/store/orchestration.nq" <<'EOF'
<https://decision-cli.dev/ns/env/ENV-001> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationEnvironment> <urn:graph:test> .
<https://decision-cli.dev/ns/env/ENV-001> <https://decision-cli.dev/ns#envType> "ephemeral-tempdir" <urn:graph:test> .
<https://decision-cli.dev/ns/result/VGR-1> <https://decision-cli.dev/ns#ranInEnvironment> <https://decision-cli.dev/ns/env/ENV-001> <urn:graph:test> .
EOF

# Pre-check: 1 VE instance present.
pre=$("$DEC" --workdir "$WORKDIR" _sparql --query \
  'PREFIX dec: <https://decision-cli.dev/ns#> SELECT (COUNT(*) AS ?n) WHERE { GRAPH ?g { ?s a dec:VerificationEnvironment } }')
if ! echo "$pre" | grep -q '"1"'; then
  echo "TC-246 FAIL: expected 1 VerificationEnvironment pre-migration, got: $pre" >&2
  exit 1
fi

# Run the migration.
"$DEC" --workdir "$WORKDIR" migrate env-to-bench >/dev/null

# Post-check 1: zero VE instances.
post_ve=$("$DEC" --workdir "$WORKDIR" _sparql --query \
  'PREFIX dec: <https://decision-cli.dev/ns#> SELECT (COUNT(*) AS ?n) WHERE { GRAPH ?g { ?s a dec:VerificationEnvironment } }')
if ! echo "$post_ve" | grep -q '"0"'; then
  echo "TC-246 FAIL: residual VerificationEnvironment after migration: $post_ve" >&2
  exit 1
fi

# Post-check 2: one VB instance with the migrated subject IRI.
post_vb=$("$DEC" --workdir "$WORKDIR" _sparql --query \
  'PREFIX dec: <https://decision-cli.dev/ns#> SELECT ?s WHERE { GRAPH ?g { ?s a dec:VerificationBench } }')
if ! echo "$post_vb" | grep -q "bench/BNCH-001"; then
  echo "TC-246 FAIL: expected bench/BNCH-001 after migration, got: $post_vb" >&2
  exit 1
fi

# Post-check 3: predicate dec:benchType present, dec:envType absent.
if ! "$DEC" --workdir "$WORKDIR" _sparql --query \
  'PREFIX dec: <https://decision-cli.dev/ns#> ASK { GRAPH ?g { ?s dec:benchType "ephemeral-tempdir" } }' \
  | grep -q "true"; then
  echo "TC-246 FAIL: dec:benchType predicate not found post-migration" >&2
  exit 1
fi
if "$DEC" --workdir "$WORKDIR" _sparql --query \
  'PREFIX dec: <https://decision-cli.dev/ns#> ASK { GRAPH ?g { ?s dec:envType ?o } }' \
  | grep -q "true"; then
  echo "TC-246 FAIL: residual dec:envType predicate post-migration" >&2
  exit 1
fi

# Post-check 4: predicate dec:ranOnBench present pointing at bench/BNCH-001.
if ! "$DEC" --workdir "$WORKDIR" _sparql --query \
  'PREFIX dec: <https://decision-cli.dev/ns#> ASK { GRAPH ?g { ?vgr dec:ranOnBench <https://decision-cli.dev/ns/bench/BNCH-001> } }' \
  | grep -q "true"; then
  echo "TC-246 FAIL: dec:ranOnBench not pointing at bench/BNCH-001" >&2
  exit 1
fi

echo "TC-246 PASS: migrate env-to-bench rewrites class + predicates + instance IRIs"
exit 0
