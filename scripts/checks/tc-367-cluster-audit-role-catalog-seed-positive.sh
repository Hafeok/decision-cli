#!/usr/bin/env bash
# TC-367 / FT-144 — positive: iri_constants ↔ seed_quad_function +
# fail-closed test present.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT="$REPO_ROOT/scripts/checks/cluster-audit-extend-role-catalog-seed.py"
FIX="$(mktemp -d -t tc-367-XXXXXX)"
trap 'rm -rf "$FIX"' EXIT

cat > "$FIX/iri_constants.rs" <<'RS'
pub const NEW_ROLE_IRI: &str = "https://decision-cli.dev/ns/role/new";
pub const NEW_AUTHORITY_IRI: &str = "https://decision-cli.dev/ns/authority/new";
RS
cat > "$FIX/seed_quad_function.rs" <<'RS'
pub fn new_role_seed_quads() -> Vec<Quad> {
    let role = NamedNode::new_unchecked(NEW_ROLE_IRI);
    let authority = NamedNode::new_unchecked(NEW_AUTHORITY_IRI);
    vec![]
}
RS
cat > "$FIX/round_trip_tests.rs" <<'RS'
#[test]
fn legacy_store_lookup_returns_safe_default() {
    // ADR-069 fail-closed lock-in
    assert!(true);
}
RS

OUT="$(python3 "$AUDIT" "$FIX" 2>&1)"
grep -q "^PASS extend-role-catalog-seed" <<<"$OUT" \
  || { echo "TC-367 FAIL: $OUT" >&2; exit 1; }
echo "TC-367 PASS"
