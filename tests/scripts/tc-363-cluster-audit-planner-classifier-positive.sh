#!/usr/bin/env bash
# TC-363 / FT-143 — positive: all 6 cells consistent.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT="$REPO_ROOT/tests/scripts/cluster-audit-extend-planner-classifier.sh"
FIX="$(mktemp -d -t tc-363-XXXXXX)"
trap 'rm -rf "$FIX"' EXIT

cat > "$FIX/inspector_trait_method.rs" <<'RS'
trait GraphInspector {
    fn check_new_signal(&self, feature_id: &str) -> Result<bool, InspectError>;
}
RS
cat > "$FIX/inspector_default_impl.rs" <<'RS'
impl GraphInspector for DefaultInspector {
    fn check_new_signal(&self, _: &str) -> Result<bool, InspectError> { Ok(false) }
}
RS
cat > "$FIX/inspector_production_impl.rs" <<'RS'
fn check_new_signal(&self, feature_id: &str) -> Result<bool, InspectError> {
    Ok(true)
}
RS
cat > "$FIX/classifier_row.rs" <<'RS'
if self.inspector.check_new_signal(feature_id)? {
    return Ok(Action::Done);
}
RS
echo "new_signal" > "$FIX/_signal_name"
cat > "$FIX/state_hash_update.rs" <<'RS'
fn classify_and_hash(...) {
    let signal = self.inspector.check_new_signal(feature_id)?;
    let mut hasher = DefaultHasher::new();
    hasher.write(if signal { b"1" } else { b"0" });
    let new_signal_hash = hasher.finish();
}
RS
cat > "$FIX/unit_tests.rs" <<'RS'
#[test]
fn precedence_new_row() {}
#[test]
fn positive_new_signal_fires() {}
#[test]
fn negative_no_signal() {}
#[test]
fn state_hash_changes_on_new_signal() {}
RS

OUT="$(bash "$AUDIT" "$FIX" 2>&1)"
grep -q "^PASS extend-planner-classifier" <<<"$OUT" \
  || { echo "TC-363 FAIL: $OUT" >&2; exit 1; }
echo "TC-363 PASS"
