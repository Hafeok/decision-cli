#!/usr/bin/env bash
# TC-357 / FT-141 — discriminator: a stray .py file in the fixture
# triggers the no_python_files firewall.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT="$REPO_ROOT/scripts/checks/cluster-audit-add-artifact-type.py"
FIX="$(mktemp -d -t tc-357-XXXXXX)"
trap 'rm -rf "$FIX"' EXIT

cat > "$FIX/example_struct.rs" <<'RS'
pub struct Example { pub id: String, }
RS
cat > "$FIX/example_shape.ttl" <<'TTL'
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix dec: <https://decision-cli.dev/ns#> .
dec:ExampleShape a sh:NodeShape ; sh:property [ sh:path dec:id ; sh:minCount 1 ] .
TTL
# Stray Python — discriminator fires.
cat > "$FIX/agent_loop.py" <<'PY'
def loop(): pass
PY

set +e
ERR="$(python3 "$AUDIT" "$FIX" 2>&1 >/dev/null)"
CODE=$?
set -e
[[ "$CODE" -eq 0 ]] && { echo "TC-357 FAIL: audit accepted negative" >&2; exit 1; }
grep -q "check=no_python_files" <<<"$ERR" \
  || { echo "TC-357 FAIL: wrong check id; got: $ERR" >&2; exit 1; }
grep -qE "add-judge-worker|add-author-worker" <<<"$ERR" \
  || { echo "TC-357 FAIL: discriminator hint absent; got: $ERR" >&2; exit 1; }
echo "TC-357 PASS"
