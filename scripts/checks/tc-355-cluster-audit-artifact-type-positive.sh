#!/usr/bin/env bash
# TC-355 / FT-141 — positive: artifact-type audit accepts Rust+TTL fixture.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT="$REPO_ROOT/scripts/checks/cluster-audit-add-artifact-type.py"
FIX="$(mktemp -d -t tc-355-XXXXXX)"
trap 'rm -rf "$FIX"' EXIT

cat > "$FIX/example_struct.rs" <<'RS'
pub struct Example {
    pub id: String,
    pub payload: String,
}
RS

cat > "$FIX/example_shape.ttl" <<'TTL'
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix dec: <https://decision-cli.dev/ns#> .
dec:ExampleShape a sh:NodeShape ; sh:targetClass dec:Example ;
  sh:property [ sh:path dec:id ; sh:minCount 1 ] ;
  sh:property [ sh:path dec:payload ; sh:minCount 1 ] .
TTL

OUT="$(python3 "$AUDIT" "$FIX" 2>&1)"
grep -q "^PASS add-artifact-type" <<<"$OUT" || { echo "TC-355 FAIL: $OUT" >&2; exit 1; }
echo "TC-355 PASS"
