#!/usr/bin/env bash
# TC-364 / FT-143 — negative: trait method returns Result<bool>; prod
# impl returns Result<u32>. Audit fires `type_mismatch`.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT="$REPO_ROOT/tests/scripts/cluster-audit-extend-planner-classifier.sh"
FIX="$(mktemp -d -t tc-364-XXXXXX)"
trap 'rm -rf "$FIX"' EXIT

cat > "$FIX/inspector_trait_method.rs" <<'RS'
trait GraphInspector {
    fn check_new_signal(&self, feature_id: &str) -> Result<bool, InspectError>;
}
RS
# WRONG return type — Result<u32>.
cat > "$FIX/inspector_production_impl.rs" <<'RS'
fn check_new_signal(&self, feature_id: &str) -> Result<u32, InspectError> {
    Ok(0)
}
RS
echo "new_signal" > "$FIX/_signal_name"
cat > "$FIX/state_hash_update.rs" <<'RS'
let mut hasher = DefaultHasher::new();
hasher.write(b"new_signal");
RS

set +e
ERR="$(bash "$AUDIT" "$FIX" 2>&1 >/dev/null)"
CODE=$?
set -e
[[ "$CODE" -eq 0 ]] && { echo "TC-364 FAIL: audit accepted negative" >&2; exit 1; }
grep -q "check=type_mismatch" <<<"$ERR" \
  || { echo "TC-364 FAIL: wrong check id; got: $ERR" >&2; exit 1; }
echo "TC-364 PASS"
