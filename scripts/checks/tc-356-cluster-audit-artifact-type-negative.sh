#!/usr/bin/env bash
# TC-356 / FT-141 — negative: SHACL omits a struct field; audit must
# fail with `shacl_field_coverage`.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT="$REPO_ROOT/scripts/checks/cluster-audit-add-artifact-type.py"
FIX="$(mktemp -d -t tc-356-XXXXXX)"
trap 'rm -rf "$FIX"' EXIT

cat > "$FIX/example_struct.rs" <<'RS'
pub struct Example {
    pub id: String,
    pub payload: String,
}
RS

# Shape omits `payload` — audit must catch.
cat > "$FIX/example_shape.ttl" <<'TTL'
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix dec: <https://decision-cli.dev/ns#> .
dec:ExampleShape a sh:NodeShape ; sh:targetClass dec:Example ;
  sh:property [ sh:path dec:id ; sh:minCount 1 ] .
TTL

set +e
ERR="$(python3 "$AUDIT" "$FIX" 2>&1 >/dev/null)"
CODE=$?
set -e
[[ "$CODE" -eq 0 ]] && { echo "TC-356 FAIL: audit accepted negative" >&2; exit 1; }
grep -q "check=shacl_field_coverage" <<<"$ERR" \
  || { echo "TC-356 FAIL: wrong check id; got: $ERR" >&2; exit 1; }
grep -q "payload" <<<"$ERR" \
  || { echo "TC-356 FAIL: missing field not named; got: $ERR" >&2; exit 1; }
echo "TC-356 PASS"
