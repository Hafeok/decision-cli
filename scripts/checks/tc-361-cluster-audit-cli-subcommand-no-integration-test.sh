#!/usr/bin/env bash
# TC-361 / FT-142 — discriminator: no file under
# crates/decision-cli/tests/; audit fails with `integration_test_path`.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
AUDIT="$REPO_ROOT/scripts/checks/cluster-audit-add-cli-subcommand.py"
FIX="$(mktemp -d -t tc-361-XXXXXX)"
trap 'rm -rf "$FIX"' EXIT

cat > "$FIX/args.rs" <<'RS'
pub struct Args { pub feature_id: String, }
RS
cat > "$FIX/handler.rs" <<'RS'
pub fn run(args: Args) {}
RS
# NO integration test — discriminator must fire.

set +e
ERR="$(python3 "$AUDIT" "$FIX" 2>&1 >/dev/null)"
CODE=$?
set -e
[[ "$CODE" -eq 0 ]] && { echo "TC-361 FAIL: audit accepted negative" >&2; exit 1; }
grep -q "check=integration_test_path" <<<"$ERR" \
  || { echo "TC-361 FAIL: wrong check id; got: $ERR" >&2; exit 1; }
echo "TC-361 PASS"
