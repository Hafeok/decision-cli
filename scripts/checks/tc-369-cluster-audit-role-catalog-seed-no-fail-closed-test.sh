#!/usr/bin/env bash
# TC-369 / FT-144 — load-bearing: round_trip_tests omits
# `legacy_store_lookup_returns_safe_default`. Audit fails with
# `fail_closed_lock_in` (the ADR-069 guarantee lock-in).
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT="$REPO_ROOT/scripts/checks/cluster-audit-extend-role-catalog-seed.py"
FIX="$(mktemp -d -t tc-369-XXXXXX)"
trap 'rm -rf "$FIX"' EXIT

cat > "$FIX/iri_constants.rs" <<'RS'
pub const NEW_ROLE_IRI: &str = "https://decision-cli.dev/ns/role/new";
RS
cat > "$FIX/seed_quad_function.rs" <<'RS'
pub fn new_role_seed_quads() -> Vec<Quad> { let _ = NEW_ROLE_IRI; vec![] }
RS
# Omits the legacy_store_lookup_returns_safe_default test.
cat > "$FIX/round_trip_tests.rs" <<'RS'
#[test]
fn some_other_test() { assert!(true); }
RS

set +e
ERR="$(python3 "$AUDIT" "$FIX" 2>&1 >/dev/null)"
CODE=$?
set -e
[[ "$CODE" -eq 0 ]] && { echo "TC-369 FAIL: audit accepted negative" >&2; exit 1; }
grep -q "check=fail_closed_lock_in" <<<"$ERR" \
  || { echo "TC-369 FAIL: wrong check id; got: $ERR" >&2; exit 1; }
grep -q "ADR-069" <<<"$ERR" \
  || { echo "TC-369 FAIL: ADR-069 not cited; got: $ERR" >&2; exit 1; }
echo "TC-369 PASS"
