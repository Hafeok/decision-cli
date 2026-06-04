#!/usr/bin/env bash
# TC-368 / FT-144 — negative: iri constant declared but not referenced
# in seed_quad_function. Audit fails with `iri_constant_reachability`.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT="$REPO_ROOT/scripts/checks/cluster-audit-extend-role-catalog-seed.py"
FIX="$(mktemp -d -t tc-368-XXXXXX)"
trap 'rm -rf "$FIX"' EXIT

cat > "$FIX/iri_constants.rs" <<'RS'
pub const NEW_ROLE_IRI: &str = "https://decision-cli.dev/ns/role/new";
pub const UNUSED_IRI: &str = "https://decision-cli.dev/ns/orphan";
RS
# UNUSED_IRI never referenced — audit fires.
cat > "$FIX/seed_quad_function.rs" <<'RS'
pub fn new_role_seed_quads() -> Vec<Quad> {
    let _r = NEW_ROLE_IRI; vec![]
}
RS
cat > "$FIX/round_trip_tests.rs" <<'RS'
#[test]
fn legacy_store_lookup_returns_safe_default() { assert!(true); }
RS

set +e
ERR="$(python3 "$AUDIT" "$FIX" 2>&1 >/dev/null)"
CODE=$?
set -e
[[ "$CODE" -eq 0 ]] && { echo "TC-368 FAIL: audit accepted negative" >&2; exit 1; }
grep -q "check=iri_constant_reachability" <<<"$ERR" \
  || { echo "TC-368 FAIL: wrong check id; got: $ERR" >&2; exit 1; }
grep -q "UNUSED_IRI" <<<"$ERR" \
  || { echo "TC-368 FAIL: orphan iri not named; got: $ERR" >&2; exit 1; }
echo "TC-368 PASS"
