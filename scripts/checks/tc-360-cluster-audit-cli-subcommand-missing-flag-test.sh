#!/usr/bin/env bash
# TC-360 / FT-142 — negative: integration test omits one advertised
# flag; audit fails with `flags_tested`.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT="$REPO_ROOT/scripts/checks/cluster-audit-add-cli-subcommand.py"
FIX="$(mktemp -d -t tc-360-XXXXXX)"
trap 'rm -rf "$FIX"' EXIT

mkdir -p "$FIX/crates/decision-cli/tests"
cat > "$FIX/args.rs" <<'RS'
pub struct Args {
    pub feature_id: String,
    pub bench: String,
}
RS
cat > "$FIX/handler.rs" <<'RS'
pub fn run(args: Args) -> ExitCode { ExitCode::SUCCESS }
RS
# Integration test only mentions feature_id; bench untested.
cat > "$FIX/crates/decision-cli/tests/integration.rs" <<'RS'
#[test]
fn partial() { let _ = "--feature-id FT-1"; }
RS

set +e
ERR="$(python3 "$AUDIT" "$FIX" 2>&1 >/dev/null)"
CODE=$?
set -e
[[ "$CODE" -eq 0 ]] && { echo "TC-360 FAIL: audit accepted negative" >&2; exit 1; }
grep -q "check=flags_tested" <<<"$ERR" \
  || { echo "TC-360 FAIL: wrong check id; got: $ERR" >&2; exit 1; }
grep -q "bench" <<<"$ERR" \
  || { echo "TC-360 FAIL: untested flag not named; got: $ERR" >&2; exit 1; }
echo "TC-360 PASS"
