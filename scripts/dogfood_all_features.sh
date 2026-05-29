#!/usr/bin/env bash
# Iterate every feature, retire its non-passing existing graphs, then
# drive-ship it. Records verdicts to /tmp/dogfood-results-<ts>.tsv.
#
# Limits:
#   --max-iter 6 gives the state-hash cycle detector room to spot
#     cycles of period ≤ 5 before the cap fires; period-≤-2 cycles
#     are still caught by the pairwise no-progress detector first.
#   timeout 600 backstops a hung verify pass per feature.

set -u

WORKDIR=${WORKDIR:-$(pwd)}
ENV_ID=${ENV_ID:-ENV-002}
TS=$(date +%s)
LOG="/tmp/dogfood-results-${TS}.tsv"
DETAIL="/tmp/dogfood-detail-${TS}.log"
RETIRED_BY="urn:dec:retired-stale-dogfood-${TS}"

echo -e "feature\toutcome\tdetail" > "$LOG"
echo "# dogfood run starting at $(date)" > "$DETAIL"

# Feature list: every FT-XXX with declared TCs that has at least one
# non-superseded graph today (so the loop has something to evaluate).
FEATURES=$(product feature list 2>/dev/null | awk 'NR>2 && /^FT-/{print $1}' | sort -u)

retire_failing_graphs_for_feature() {
  local ft="$1"
  local feature_iri="https://decision-cli.dev/ns/feature/${ft}"
  # SPARQL: every non-superseded VG that claims to verify this feature
  # AND whose latest VGR isn't approved. Retire by superseding with
  # the run's sentinel IRI.
  local query
  query=$(cat <<EOF
PREFIX dec: <https://decision-cli.dev/ns#>
SELECT DISTINCT ?graph WHERE {
  GRAPH ?g1 { ?graph dec:verifies <${feature_iri}> . }
  FILTER NOT EXISTS { GRAPH ?gs { ?graph dec:supersededBy ?_succ } }
  FILTER EXISTS {
    GRAPH ?g2 { ?vgr dec:resultOf ?graph ; dec:verdict ?verdict . }
    FILTER(?verdict != "approved")
  }
}
EOF
)
  local vgs
  vgs=$(dec _sparql --workdir "$WORKDIR" --query "$query" 2>/dev/null \
    | awk -F'\t' '{print $1}' \
    | sed 's/?graph=<\(.*\)>/\1/')
  local n=0
  for vg_iri in $vgs; do
    local vg_short="${vg_iri##*/}"
    dec --workdir "$WORKDIR" _supersede-graph "$vg_short" --by "$RETIRED_BY" >/dev/null 2>&1 \
      && n=$((n+1))
  done
  echo "$n"
}

declare -i done_count=0 stuck_count=0 timeout_count=0 other_count=0

for ft in $FEATURES; do
  echo "=== $ft @ $(date +%H:%M:%S) ===" | tee -a "$DETAIL"
  retired=$(retire_failing_graphs_for_feature "$ft")
  echo "retired $retired failing graphs" | tee -a "$DETAIL"

  out=$(timeout 600 dec --workdir "$WORKDIR" drive ship "$ft" --env "$ENV_ID" --max-iter 6 2>&1)
  rc=$?
  echo "$out" | tee -a "$DETAIL"

  if [ "$rc" -eq 124 ]; then
    outcome="timeout"
    timeout_count=$((timeout_count+1))
    detail="timed out after 600s"
  elif echo "$out" | grep -q "reached goal"; then
    outcome="done"
    done_count=$((done_count+1))
    detail="approved"
  elif echo "$out" | grep -q "drive: stuck"; then
    outcome="stuck"
    stuck_count=$((stuck_count+1))
    detail=$(echo "$out" | grep "drive: stuck" | head -1 | sed 's/^drive: stuck — //')
  elif echo "$out" | grep -q "hit iteration cap"; then
    outcome="max-iter"
    other_count=$((other_count+1))
    detail="hit max-iter cap"
  else
    outcome="other"
    other_count=$((other_count+1))
    detail=$(echo "$out" | tail -2 | tr '\n' ' ')
  fi
  echo -e "${ft}\t${outcome}\t${detail}" >> "$LOG"
done

echo
echo "=== summary ==="
echo "done:    $done_count"
echo "stuck:   $stuck_count"
echo "timeout: $timeout_count"
echo "other:   $other_count"
echo
echo "results: $LOG"
echo "detail:  $DETAIL"
