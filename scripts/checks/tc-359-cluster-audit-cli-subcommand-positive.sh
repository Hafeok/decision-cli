#!/usr/bin/env bash
# TC-359 / FT-142 — positive: integration test under
# crates/decision-cli/tests/ + every flag referenced.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT="$REPO_ROOT/scripts/checks/cluster-audit-add-cli-subcommand.py"
FIX="$(mktemp -d -t tc-359-XXXXXX)"
trap 'rm -rf "$FIX"' EXIT

mkdir -p "$FIX/crates/decision-cli/tests"
cat > "$FIX/args.rs" <<'RS'
pub struct Args {
    pub feature_id: String,
    pub bench: String,
}
RS
cat > "$FIX/handler.rs" <<'RS'
pub fn run(args: Args) -> ExitCode {
    println!("{} {}", args.feature_id, args.bench);
    ExitCode::SUCCESS
}
RS
cat > "$FIX/crates/decision-cli/tests/integration.rs" <<'RS'
#[test]
fn exercise_flags() {
    // covers --feature-id and --bench
    let _ = "--feature-id FT-1 --bench BNCH-002";
}
RS

OUT="$(python3 "$AUDIT" "$FIX" 2>&1)"
grep -q "^PASS add-cli-subcommand" <<<"$OUT" || { echo "TC-359 FAIL: $OUT" >&2; exit 1; }
echo "TC-359 PASS"
