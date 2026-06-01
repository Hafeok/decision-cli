#!/usr/bin/env bash
# TC-265 — FT-100 unblock regression: 8 orphan defects from VG-088/VG-155
# referencing TC-162/TC-163/TC-164 (the exact shape that stuck the drive)
# are retractable in one `dec _retract-orphan-defects --feature FT-100`
# call, and a subsequent run finds zero candidates.
#
# Spec: .product/tests/TC-265-*.md
# Implements: FT-120
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-265.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT
mkdir -p "$WORKDIR/.dec/store"

# Replicate the FT-100 stuck-drive state in miniature:
#   VG-088 and VG-155 both have no steps covering TC-162, TC-163, TC-164.
#   FT-100 links to VG-088 (the canonical VG for the feature).
#   8 open defects in `produced` state are split between the two VGs.
ORC="https://decision-cli.dev/ns/orchestration"

emit_fb() {
    local fb_iri="$1" tc_iri="$2" session_iri="$3"
    cat <<EOF
<$fb_iri> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feedback> <$ORC> .
<$fb_iri> <https://decision-cli.dev/ns#sourceArtifact> <$tc_iri> <$ORC> .
<$fb_iri> <https://decision-cli.dev/ns#sourceSession> <$session_iri> <$ORC> .
<$fb_iri> <https://decision-cli.dev/ns#lifecycleState> "produced" <$ORC> .
EOF
}

{
cat <<EOF
<https://decision-cli.dev/ns/stream/tc-265> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#ValueStream> .
<https://decision-cli.dev/ns/stream/tc-265> <https://decision-cli.dev/ns#terminalValueAction> <https://decision-cli.dev/ns/value-actions/test-action> .
<https://decision-cli.dev/ns/feature/FT-100> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feature> <$ORC> .
<https://decision-cli.dev/ns/verify/graph/VG-088> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationGraph> <$ORC> .
<https://decision-cli.dev/ns/verify/graph/VG-088> <https://decision-cli.dev/ns#verifies> <https://decision-cli.dev/ns/feature/FT-100> <$ORC> .
<https://decision-cli.dev/ns/verify/graph/VG-088> <https://decision-cli.dev/ns#steps> <http://www.w3.org/1999/02/22-rdf-syntax-ns#nil> <$ORC> .
<https://decision-cli.dev/ns/verify/graph/VG-155> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationGraph> <$ORC> .
<https://decision-cli.dev/ns/verify/graph/VG-155> <https://decision-cli.dev/ns#verifies> <https://decision-cli.dev/ns/feature/FT-100> <$ORC> .
<https://decision-cli.dev/ns/verify/graph/VG-155> <https://decision-cli.dev/ns#steps> <http://www.w3.org/1999/02/22-rdf-syntax-ns#nil> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-088> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationGraphResult> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-088> <https://decision-cli.dev/ns#resultOf> <https://decision-cli.dev/ns/verify/graph/VG-088> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-088> <http://www.w3.org/ns/prov#wasGeneratedBy> <https://decision-cli.dev/ns/session/sess-088> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-155> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationGraphResult> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-155> <https://decision-cli.dev/ns#resultOf> <https://decision-cli.dev/ns/verify/graph/VG-155> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-155> <http://www.w3.org/ns/prov#wasGeneratedBy> <https://decision-cli.dev/ns/session/sess-155> <$ORC> .
EOF
# 5 defects from VG-088
emit_fb "https://test/fb-088-1" "https://decision-cli.dev/ns/test/TC-162" "https://decision-cli.dev/ns/session/sess-088"
emit_fb "https://test/fb-088-2" "https://decision-cli.dev/ns/test/TC-163" "https://decision-cli.dev/ns/session/sess-088"
emit_fb "https://test/fb-088-3" "https://decision-cli.dev/ns/test/TC-164" "https://decision-cli.dev/ns/session/sess-088"
emit_fb "https://test/fb-088-4" "https://decision-cli.dev/ns/test/TC-162" "https://decision-cli.dev/ns/session/sess-088"
emit_fb "https://test/fb-088-5" "https://decision-cli.dev/ns/test/TC-163" "https://decision-cli.dev/ns/session/sess-088"
# 3 defects from VG-155
emit_fb "https://test/fb-155-1" "https://decision-cli.dev/ns/test/TC-162" "https://decision-cli.dev/ns/session/sess-155"
emit_fb "https://test/fb-155-2" "https://decision-cli.dev/ns/test/TC-163" "https://decision-cli.dev/ns/session/sess-155"
emit_fb "https://test/fb-155-3" "https://decision-cli.dev/ns/test/TC-164" "https://decision-cli.dev/ns/session/sess-155"
} > "$WORKDIR/.dec/store/orchestration.nq"

# --- Pre-state: 8 open defects ---
pre=$("$DEC" --workdir "$WORKDIR" _sparql --query \
    'PREFIX dec: <https://decision-cli.dev/ns#> SELECT (COUNT(*) AS ?n) WHERE { GRAPH ?g { ?fb a dec:Feedback ; dec:lifecycleState "produced" } }')
if ! echo "$pre" | grep -q '"8"'; then
    echo "TC-265 FAIL: expected 8 open defects pre-retract, got: $pre" >&2
    exit 1
fi

# --- Retract via --feature FT-100 ---
out=$("$DEC" --workdir "$WORKDIR" _retract-orphan-defects --feature FT-100)
if ! printf '%s\n' "$out" | grep -q "total: 8 retracted"; then
    echo "TC-265 FAIL: retract pass did not report 8 retractions:" >&2
    printf '%s\n' "$out" | sed 's/^/    /' >&2
    exit 1
fi

# --- Post-state: 0 open defects, 8 superseded with topology-change predicate ---
post_open=$("$DEC" --workdir "$WORKDIR" _sparql --query \
    'PREFIX dec: <https://decision-cli.dev/ns#> SELECT (COUNT(*) AS ?n) WHERE { GRAPH ?g { ?fb a dec:Feedback ; dec:lifecycleState "produced" } }')
if ! echo "$post_open" | grep -q '"0"'; then
    echo "TC-265 FAIL: expected 0 open defects post-retract, got: $post_open" >&2
    exit 1
fi
post_super=$("$DEC" --workdir "$WORKDIR" _sparql --query \
    'PREFIX dec: <https://decision-cli.dev/ns#> SELECT (COUNT(*) AS ?n) WHERE { GRAPH ?g { ?fb a dec:Feedback ; dec:lifecycleState "superseded" ; dec:supersededByTopologyChange ?s } }')
if ! echo "$post_super" | grep -q '"8"'; then
    echo "TC-265 FAIL: expected 8 superseded defects with topology-change predicate, got: $post_super" >&2
    exit 1
fi

# --- Idempotency: rerun finds 0 ---
again=$("$DEC" --workdir "$WORKDIR" _retract-orphan-defects --feature FT-100)
if ! printf '%s\n' "$again" | grep -q "No orphan defects found"; then
    echo "TC-265 FAIL: rerun did not report 'No orphan defects found':" >&2
    printf '%s\n' "$again" | sed 's/^/    /' >&2
    exit 1
fi

echo "TC-265 PASS"
exit 0
