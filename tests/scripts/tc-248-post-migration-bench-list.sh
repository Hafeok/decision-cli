#!/usr/bin/env bash
# TC-248 — Post-migration dec verify bench list returns the migrated
# entries with BNCH ids.
#
# Spec: .product/tests/TC-248-*.md
# Implements: FT-117

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-248.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT
mkdir -p "$WORKDIR/.dec/store"

# Seed three pre-rename ENV instances in the legacy `verify-env`
# named graph — the production layout `dec init` used pre-FT-112.
# The migration moves quads to `verify-bench` where `dec verify bench
# list` queries.
cat > "$WORKDIR/.dec/store/orchestration.nq" <<'EOF'
<https://decision-cli.dev/ns/env/ENV-001> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationEnvironment> <https://decision-cli.dev/ns/graph/verify-env> .
<https://decision-cli.dev/ns/env/ENV-001> <https://decision-cli.dev/ns#envType> "ephemeral-tempdir" <https://decision-cli.dev/ns/graph/verify-env> .
<https://decision-cli.dev/ns/env/ENV-001> <https://decision-cli.dev/ns#safetyClass> "isolated" <https://decision-cli.dev/ns/graph/verify-env> .
<https://decision-cli.dev/ns/env/ENV-002> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationEnvironment> <https://decision-cli.dev/ns/graph/verify-env> .
<https://decision-cli.dev/ns/env/ENV-002> <https://decision-cli.dev/ns#envType> "ephemeral-tempdir" <https://decision-cli.dev/ns/graph/verify-env> .
<https://decision-cli.dev/ns/env/ENV-002> <https://decision-cli.dev/ns#safetyClass> "isolated" <https://decision-cli.dev/ns/graph/verify-env> .
<https://decision-cli.dev/ns/env/ENV-100> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationEnvironment> <https://decision-cli.dev/ns/graph/verify-env> .
<https://decision-cli.dev/ns/env/ENV-100> <https://decision-cli.dev/ns#envType> "local-cli" <https://decision-cli.dev/ns/graph/verify-env> .
<https://decision-cli.dev/ns/env/ENV-100> <https://decision-cli.dev/ns#safetyClass> "isolated" <https://decision-cli.dev/ns/graph/verify-env> .
EOF

# Pre-check: bench list is empty (the bug condition).
pre=$("$DEC" --workdir "$WORKDIR" verify bench list 2>&1)
if ! echo "$pre" | grep -q "no benches yet"; then
  echo "TC-248 FAIL: expected 'no benches yet' pre-migration, got: $pre" >&2
  exit 1
fi

# Run migration.
"$DEC" --workdir "$WORKDIR" migrate env-to-bench >/dev/null

# Post-check: bench list shows all three with renamed ids.
post=$("$DEC" --workdir "$WORKDIR" verify bench list 2>&1)
for id in BNCH-001 BNCH-002 BNCH-100; do
  if ! echo "$post" | grep -q "$id"; then
    echo "TC-248 FAIL: $id missing from bench list, got: $post" >&2
    exit 1
  fi
done
# Negative check: old ids should not appear.
for id in ENV-001 ENV-002 ENV-100; do
  if echo "$post" | grep -q "$id"; then
    echo "TC-248 FAIL: legacy $id should not appear in bench list, got: $post" >&2
    exit 1
  fi
done

echo "TC-248 PASS: post-migration bench list returns BNCH-001/002/100"
exit 0
