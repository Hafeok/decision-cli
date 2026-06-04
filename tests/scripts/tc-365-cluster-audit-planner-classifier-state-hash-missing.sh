#!/usr/bin/env bash
# TC-365 / FT-143 — load-bearing: state_hash_update.rs does not fold
# the new signal name into the hasher. Audit fires `state_hash_missing`
# (the FT-138 TC-349 silent-regression guard, generalised).
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT="$REPO_ROOT/tests/scripts/cluster-audit-extend-planner-classifier.sh"
FIX="$(mktemp -d -t tc-365-XXXXXX)"
trap 'rm -rf "$FIX"' EXIT

cat > "$FIX/inspector_trait_method.rs" <<'RS'
trait GraphInspector {
    fn check_new_signal(&self, feature_id: &str) -> Result<bool, InspectError>;
}
RS
cat > "$FIX/inspector_production_impl.rs" <<'RS'
fn check_new_signal(&self, feature_id: &str) -> Result<bool, InspectError> { Ok(true) }
RS
echo "new_signal" > "$FIX/_signal_name"
# state_hash_update does hash other things but NOT the new signal.
cat > "$FIX/state_hash_update.rs" <<'RS'
let mut hasher = DefaultHasher::new();
hasher.write(b"old_signal");
RS

set +e
ERR="$(bash "$AUDIT" "$FIX" 2>&1 >/dev/null)"
CODE=$?
set -e
[[ "$CODE" -eq 0 ]] && { echo "TC-365 FAIL: audit accepted negative" >&2; exit 1; }
grep -q "check=state_hash_missing" <<<"$ERR" \
  || { echo "TC-365 FAIL: wrong check id; got: $ERR" >&2; exit 1; }
grep -q "new_signal" <<<"$ERR" \
  || { echo "TC-365 FAIL: signal name not cited; got: $ERR" >&2; exit 1; }
echo "TC-365 PASS"
