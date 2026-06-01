#!/usr/bin/env bash
# TC-263 — `dec _retract-orphan-defects --all` sweeps every graph;
# `--feature FT-XXX` restricts to graphs linked to the feature.
#
# Spec: .product/tests/TC-263-*.md
# Implements: FT-120
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

cargo build --quiet --package decision-cli --bin dec
DEC="$REPO_ROOT/target/debug/dec"

WORKDIR="$(mktemp -d --tmpdir tc-263.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT
mkdir -p "$WORKDIR/.dec/store"

# Fixture: 3 graphs.
#   VG-A — 2 orphan defects, linked to FT-263.
#   VG-B — 0 orphan defects (one TC, covered by one step).
#   VG-C — 5 orphan defects, NOT linked to FT-263.
ORC="https://decision-cli.dev/ns/orchestration"

cat > "$WORKDIR/.dec/store/orchestration.nq" <<EOF
<https://decision-cli.dev/ns/stream/tc-263> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#ValueStream> .
<https://decision-cli.dev/ns/stream/tc-263> <https://decision-cli.dev/ns#terminalValueAction> <https://decision-cli.dev/ns/value-actions/test-action> .
<https://decision-cli.dev/ns/feature/FT-263> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feature> <$ORC> .
<https://decision-cli.dev/ns/verify/graph/VG-A> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationGraph> <$ORC> .
<https://decision-cli.dev/ns/verify/graph/VG-A> <https://decision-cli.dev/ns#verifies> <https://decision-cli.dev/ns/feature/FT-263> <$ORC> .
<https://decision-cli.dev/ns/verify/graph/VG-A> <https://decision-cli.dev/ns#steps> <http://www.w3.org/1999/02/22-rdf-syntax-ns#nil> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-A> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationGraphResult> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-A> <https://decision-cli.dev/ns#resultOf> <https://decision-cli.dev/ns/verify/graph/VG-A> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-A> <http://www.w3.org/ns/prov#wasGeneratedBy> <https://test/sess-a> <$ORC> .
<https://test/fb-a1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feedback> <$ORC> .
<https://test/fb-a1> <https://decision-cli.dev/ns#sourceArtifact> <https://decision-cli.dev/ns/test/TC-a1> <$ORC> .
<https://test/fb-a1> <https://decision-cli.dev/ns#sourceSession> <https://test/sess-a> <$ORC> .
<https://test/fb-a1> <https://decision-cli.dev/ns#lifecycleState> "produced" <$ORC> .
<https://test/fb-a2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feedback> <$ORC> .
<https://test/fb-a2> <https://decision-cli.dev/ns#sourceArtifact> <https://decision-cli.dev/ns/test/TC-a2> <$ORC> .
<https://test/fb-a2> <https://decision-cli.dev/ns#sourceSession> <https://test/sess-a> <$ORC> .
<https://test/fb-a2> <https://decision-cli.dev/ns#lifecycleState> "produced" <$ORC> .
<https://decision-cli.dev/ns/verify/graph/VG-B> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationGraph> <$ORC> .
_:b0 <http://www.w3.org/1999/02/22-rdf-syntax-ns#first> <https://decision-cli.dev/ns/verify/step/STEP-B1> <$ORC> .
_:b0 <http://www.w3.org/1999/02/22-rdf-syntax-ns#rest> <http://www.w3.org/1999/02/22-rdf-syntax-ns#nil> <$ORC> .
<https://decision-cli.dev/ns/verify/graph/VG-B> <https://decision-cli.dev/ns#steps> _:b0 <$ORC> .
<https://decision-cli.dev/ns/verify/step/STEP-B1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationStep> <$ORC> .
<https://decision-cli.dev/ns/verify/step/STEP-B1> <https://decision-cli.dev/ns#providesEvidenceFor> <https://decision-cli.dev/ns/test/TC-b1> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-B> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationGraphResult> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-B> <https://decision-cli.dev/ns#resultOf> <https://decision-cli.dev/ns/verify/graph/VG-B> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-B> <http://www.w3.org/ns/prov#wasGeneratedBy> <https://test/sess-b> <$ORC> .
<https://test/fb-b1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feedback> <$ORC> .
<https://test/fb-b1> <https://decision-cli.dev/ns#sourceArtifact> <https://decision-cli.dev/ns/test/TC-b1> <$ORC> .
<https://test/fb-b1> <https://decision-cli.dev/ns#sourceSession> <https://test/sess-b> <$ORC> .
<https://test/fb-b1> <https://decision-cli.dev/ns#lifecycleState> "produced" <$ORC> .
<https://decision-cli.dev/ns/verify/graph/VG-C> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationGraph> <$ORC> .
<https://decision-cli.dev/ns/verify/graph/VG-C> <https://decision-cli.dev/ns#steps> <http://www.w3.org/1999/02/22-rdf-syntax-ns#nil> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-C> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#VerificationGraphResult> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-C> <https://decision-cli.dev/ns#resultOf> <https://decision-cli.dev/ns/verify/graph/VG-C> <$ORC> .
<https://decision-cli.dev/ns/result/VGR-C> <http://www.w3.org/ns/prov#wasGeneratedBy> <https://test/sess-c> <$ORC> .
<https://test/fb-c1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feedback> <$ORC> .
<https://test/fb-c1> <https://decision-cli.dev/ns#sourceArtifact> <https://decision-cli.dev/ns/test/TC-c1> <$ORC> .
<https://test/fb-c1> <https://decision-cli.dev/ns#sourceSession> <https://test/sess-c> <$ORC> .
<https://test/fb-c1> <https://decision-cli.dev/ns#lifecycleState> "produced" <$ORC> .
<https://test/fb-c2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feedback> <$ORC> .
<https://test/fb-c2> <https://decision-cli.dev/ns#sourceArtifact> <https://decision-cli.dev/ns/test/TC-c2> <$ORC> .
<https://test/fb-c2> <https://decision-cli.dev/ns#sourceSession> <https://test/sess-c> <$ORC> .
<https://test/fb-c2> <https://decision-cli.dev/ns#lifecycleState> "produced" <$ORC> .
<https://test/fb-c3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feedback> <$ORC> .
<https://test/fb-c3> <https://decision-cli.dev/ns#sourceArtifact> <https://decision-cli.dev/ns/test/TC-c3> <$ORC> .
<https://test/fb-c3> <https://decision-cli.dev/ns#sourceSession> <https://test/sess-c> <$ORC> .
<https://test/fb-c3> <https://decision-cli.dev/ns#lifecycleState> "produced" <$ORC> .
<https://test/fb-c4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feedback> <$ORC> .
<https://test/fb-c4> <https://decision-cli.dev/ns#sourceArtifact> <https://decision-cli.dev/ns/test/TC-c4> <$ORC> .
<https://test/fb-c4> <https://decision-cli.dev/ns#sourceSession> <https://test/sess-c> <$ORC> .
<https://test/fb-c4> <https://decision-cli.dev/ns#lifecycleState> "produced" <$ORC> .
<https://test/fb-c5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <https://decision-cli.dev/ns#Feedback> <$ORC> .
<https://test/fb-c5> <https://decision-cli.dev/ns#sourceArtifact> <https://decision-cli.dev/ns/test/TC-c5> <$ORC> .
<https://test/fb-c5> <https://decision-cli.dev/ns#sourceSession> <https://test/sess-c> <$ORC> .
<https://test/fb-c5> <https://decision-cli.dev/ns#lifecycleState> "produced" <$ORC> .
EOF

# --- Assertion 1: --feature FT-263 retracts only VG-A's 2 orphans ---
ft_out=$("$DEC" --workdir "$WORKDIR" _retract-orphan-defects --feature FT-263)
if ! printf '%s\n' "$ft_out" | grep -q "VG-A: 2 retracted"; then
    echo "TC-263 FAIL: --feature output missing VG-A: 2 retracted" >&2
    printf '%s\n' "$ft_out" | sed 's/^/    /' >&2
    exit 1
fi
if ! printf '%s\n' "$ft_out" | grep -q "total: 2 retracted"; then
    echo "TC-263 FAIL: --feature output missing total: 2 retracted" >&2
    exit 1
fi
# VG-C's defects must still be open.
unchanged=$("$DEC" --workdir "$WORKDIR" _sparql --query \
    'PREFIX dec: <https://decision-cli.dev/ns#> SELECT (COUNT(*) AS ?n) WHERE { GRAPH ?g { ?fb a dec:Feedback ; dec:lifecycleState "produced" } }')
if ! echo "$unchanged" | grep -q '"6"'; then
    # 6 = VG-B's 1 + VG-C's 5; VG-A's 2 are now superseded
    echo "TC-263 FAIL: expected 6 produced post-feature, got: $unchanged" >&2
    exit 1
fi

# --- Assertion 2: --all retracts the remaining 5 VG-C orphans ---
all_out=$("$DEC" --workdir "$WORKDIR" _retract-orphan-defects --all)
if ! printf '%s\n' "$all_out" | grep -q "VG-C: 5 retracted"; then
    echo "TC-263 FAIL: --all output missing VG-C: 5 retracted" >&2
    printf '%s\n' "$all_out" | sed 's/^/    /' >&2
    exit 1
fi
if ! printf '%s\n' "$all_out" | grep -q "total: 5 retracted"; then
    echo "TC-263 FAIL: --all output missing total: 5 retracted" >&2
    exit 1
fi

# --- Assertion 3: VG-B (covered TC) untouched throughout ---
vb_state=$("$DEC" --workdir "$WORKDIR" _sparql --query \
    'PREFIX dec: <https://decision-cli.dev/ns#> SELECT ?state WHERE { GRAPH ?g { <https://test/fb-b1> dec:lifecycleState ?state } }')
if ! echo "$vb_state" | grep -q '"produced"'; then
    echo "TC-263 FAIL: VG-B's covered defect was modified; expected produced, got: $vb_state" >&2
    exit 1
fi

# --- Assertion 4: dry-run version of --all reports same tally ---
# Rerun a clean fixture for the dry-run check.
sed -i 's|"superseded"|"produced"|g' "$WORKDIR/.dec/store/orchestration.nq"
# Drop the supersedeBy* predicates added by the prior live run.
"$DEC" --workdir "$WORKDIR" _sparql --query \
    'PREFIX dec: <https://decision-cli.dev/ns#> SELECT (COUNT(*) AS ?n) WHERE { GRAPH ?g { ?fb dec:supersededByTopologyChange ?x } }' > /dev/null || true

dry_all=$("$DEC" --workdir "$WORKDIR" _retract-orphan-defects --all --dry-run)
if ! printf '%s\n' "$dry_all" | grep -q "total: 7 would retract"; then
    echo "TC-263 FAIL: --all --dry-run expected total: 7 would retract, got:" >&2
    printf '%s\n' "$dry_all" | sed 's/^/    /' >&2
    exit 1
fi

# --- Assertion 5: malformed args exit 2 ---
set +e
"$DEC" --workdir "$WORKDIR" _retract-orphan-defects 2>/dev/null
rc=$?
set -e
if [ "$rc" -ne 2 ]; then
    echo "TC-263 FAIL: missing scope flag expected exit 2, got $rc" >&2
    exit 1
fi

echo "TC-263 PASS"
exit 0
