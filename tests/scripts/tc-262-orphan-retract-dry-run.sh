#!/usr/bin/env bash
# TC-262 — `dec _retract-orphan-defects --graph VG-NNN --dry-run` lists
# candidates without writing; live invocation retracts.
#
# Spec: .product/tests/TC-262-*.md
# Implements: FT-120
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-262.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT
mkdir -p "$WORKDIR/.dec/store"

# Seed minimum ValueStream (so ActiveScope::load succeeds) plus
# VG-262 with NO steps (so any defect against any TC is orphan),
# VGR-262 wasGeneratedBy session-262,
# 2 open defects (produced) and 1 terminal defect (closed) — terminal must
# survive the retraction pass untouched.
cat > "$WORKDIR/.dec/store/orchestration.nq" <<'EOF'
<https://decision-cli.dev/ns/stream/tc-262> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#ValueStream> .
<https://decision-cli.dev/ns/stream/tc-262> <https://decision-cli.dev/ns#terminalValueAction> <https://decision-cli.dev/ns/value-actions/test-action> .
<https://decision-cli.dev/ns/verify/graph/VG-262> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationGraph> <https://decision-cli.dev/ns/orchestration> .
<https://decision-cli.dev/ns/verify/graph/VG-262> <https://decision-cli.dev/ns#steps> <http://www.w3.org/1999/02/22-rdf-syntax-ns#nil> <https://decision-cli.dev/ns/orchestration> .
<https://decision-cli.dev/ns/result/VGR-262> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationGraphResult> <https://decision-cli.dev/ns/orchestration> .
<https://decision-cli.dev/ns/result/VGR-262> <https://decision-cli.dev/ns#resultOf> <https://decision-cli.dev/ns/verify/graph/VG-262> <https://decision-cli.dev/ns/orchestration> .
<https://decision-cli.dev/ns/result/VGR-262> <http://www.w3.org/ns/prov#wasGeneratedBy> <https://decision-cli.dev/ns/session/sess-262> <https://decision-cli.dev/ns/orchestration> .
<https://test/fb-a> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feedback> <https://decision-cli.dev/ns/orchestration> .
<https://test/fb-a> <https://decision-cli.dev/ns#sourceArtifact> <https://decision-cli.dev/ns/test/TC-262-a> <https://decision-cli.dev/ns/orchestration> .
<https://test/fb-a> <https://decision-cli.dev/ns#sourceSession> <https://decision-cli.dev/ns/session/sess-262> <https://decision-cli.dev/ns/orchestration> .
<https://test/fb-a> <https://decision-cli.dev/ns#lifecycleState> "produced" <https://decision-cli.dev/ns/orchestration> .
<https://test/fb-b> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feedback> <https://decision-cli.dev/ns/orchestration> .
<https://test/fb-b> <https://decision-cli.dev/ns#sourceArtifact> <https://decision-cli.dev/ns/test/TC-262-b> <https://decision-cli.dev/ns/orchestration> .
<https://test/fb-b> <https://decision-cli.dev/ns#sourceSession> <https://decision-cli.dev/ns/session/sess-262> <https://decision-cli.dev/ns/orchestration> .
<https://test/fb-b> <https://decision-cli.dev/ns#lifecycleState> "produced" <https://decision-cli.dev/ns/orchestration> .
<https://test/fb-terminal> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feedback> <https://decision-cli.dev/ns/orchestration> .
<https://test/fb-terminal> <https://decision-cli.dev/ns#sourceArtifact> <https://decision-cli.dev/ns/test/TC-262-c> <https://decision-cli.dev/ns/orchestration> .
<https://test/fb-terminal> <https://decision-cli.dev/ns#sourceSession> <https://decision-cli.dev/ns/session/sess-262> <https://decision-cli.dev/ns/orchestration> .
<https://test/fb-terminal> <https://decision-cli.dev/ns#lifecycleState> "closed" <https://decision-cli.dev/ns/orchestration> .
EOF

# --- Assertion 1: --dry-run lists exactly 2 candidates, no writes ---
dry_out=$("$DEC" --workdir "$WORKDIR" _retract-orphan-defects --graph VG-262 --dry-run)
if ! printf '%s\n' "$dry_out" | grep -q "VG-262: 2 would retract"; then
    echo "TC-262 FAIL: dry-run output missing VG-262: 2 would retract" >&2
    printf '%s\n' "$dry_out" | sed 's/^/    /' >&2
    exit 1
fi
if ! printf '%s\n' "$dry_out" | grep -q "total: 2"; then
    echo "TC-262 FAIL: dry-run output missing total: 2" >&2
    exit 1
fi
# Lifecycle states unchanged.
post_dry=$("$DEC" --workdir "$WORKDIR" _sparql --query \
    'PREFIX dec: <https://decision-cli.dev/ns#> SELECT (COUNT(*) AS ?n) WHERE { GRAPH ?g { ?fb a dec:Feedback ; dec:lifecycleState "produced" } }')
if ! echo "$post_dry" | grep -q '"2"'; then
    echo "TC-262 FAIL: dry-run mutated lifecycle states; expected 2 produced, got: $post_dry" >&2
    exit 1
fi

# --- Assertion 2: live invocation retracts ---
live_out=$("$DEC" --workdir "$WORKDIR" _retract-orphan-defects --graph VG-262)
if ! printf '%s\n' "$live_out" | grep -q "total: 2 retracted"; then
    echo "TC-262 FAIL: live output missing total: 2 retracted" >&2
    printf '%s\n' "$live_out" | sed 's/^/    /' >&2
    exit 1
fi
post_live=$("$DEC" --workdir "$WORKDIR" _sparql --query \
    'PREFIX dec: <https://decision-cli.dev/ns#> SELECT (COUNT(*) AS ?n) WHERE { GRAPH ?g { ?fb a dec:Feedback ; dec:lifecycleState "superseded" ; dec:supersededByTopologyChange ?s } }')
if ! echo "$post_live" | grep -q '"2"'; then
    echo "TC-262 FAIL: expected 2 superseded defects with topology-change predicate, got: $post_live" >&2
    exit 1
fi
# Closed defect untouched.
closed_count=$("$DEC" --workdir "$WORKDIR" _sparql --query \
    'PREFIX dec: <https://decision-cli.dev/ns#> SELECT (COUNT(*) AS ?n) WHERE { GRAPH ?g { ?fb a dec:Feedback ; dec:lifecycleState "closed" } }')
if ! echo "$closed_count" | grep -q '"1"'; then
    echo "TC-262 FAIL: closed defect was modified; expected 1 closed, got: $closed_count" >&2
    exit 1
fi

# --- Assertion 3: idempotent rerun finds 0 ---
again=$("$DEC" --workdir "$WORKDIR" _retract-orphan-defects --graph VG-262)
if ! printf '%s\n' "$again" | grep -q "No orphan defects found"; then
    echo "TC-262 FAIL: rerun did not report 'No orphan defects found':" >&2
    printf '%s\n' "$again" | sed 's/^/    /' >&2
    exit 1
fi

# --- Assertion 4: hidden from help ---
if "$DEC" --help 2>&1 | grep -q "_retract-orphan-defects"; then
    echo "TC-262 FAIL: _retract-orphan-defects appears in --help (should be hidden)" >&2
    exit 1
fi

echo "TC-262 PASS"
exit 0
