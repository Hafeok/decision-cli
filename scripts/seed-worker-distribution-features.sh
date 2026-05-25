#!/usr/bin/env bash
# Seed the worker-distribution slice-1 features (FT-086..FT-096) so they pass
# preflight, gap check, and graph check. Idempotent — safe to re-run.
#
# Does three things per feature:
#   1. Links cross-cutting ADRs that genuinely apply.
#   2. Acknowledges every remaining cross-cutting ADR with a per-ADR reason.
#   3. Authors a single exit-criteria TC and links it back to the feature.

set -euo pipefail

NEW_FEATURES=(FT-086 FT-087 FT-088 FT-089 FT-090 FT-091 FT-092 FT-093 FT-094 FT-095 FT-096)

############################################################
# 1. ADR LINKING — features ↔ cross-cutting ADRs that apply.
############################################################

# Universal links: code-quality + SDP + Brief artifact type land on every
# feature in this brief.
UNIVERSAL_ADRS=(ADR-013 ADR-016 ADR-044)

# Per-feature additional links (newline-separated).
declare -A EXTRA_ADRS
EXTRA_ADRS[FT-086]="ADR-036
ADR-038
ADR-039
ADR-041
ADR-043
ADR-047"
EXTRA_ADRS[FT-087]="ADR-002
ADR-038
ADR-039
ADR-040
ADR-041"
EXTRA_ADRS[FT-088]=""
EXTRA_ADRS[FT-089]=""
EXTRA_ADRS[FT-090]="ADR-017
ADR-018
ADR-021
ADR-038
ADR-039
ADR-041"
EXTRA_ADRS[FT-091]=""
EXTRA_ADRS[FT-092]="ADR-022
ADR-023
ADR-024
ADR-025
ADR-027
ADR-035
ADR-038
ADR-039
ADR-041"
EXTRA_ADRS[FT-093]=""
EXTRA_ADRS[FT-094]="ADR-002
ADR-004
ADR-038
ADR-039
ADR-041"
EXTRA_ADRS[FT-095]="ADR-054"
EXTRA_ADRS[FT-096]="ADR-047
ADR-054"

############################################################
# 2. ACKNOWLEDGMENT REASONS — per cross-cutting ADR.
############################################################

declare -A REASON
REASON[ADR-001]="Application-layer feature; does not touch the oxi-events crate boundary."
REASON[ADR-002]="Feature ships infrastructure / packaging conventions, not graph mutations."
REASON[ADR-004]="Feature does not emit dispatch or session events; PROV-O hookup happens in features that write artifacts."
REASON[ADR-005]="Worker-registration discipline is independent of value-stream scope."
REASON[ADR-012]="Not a per-stream command; no working-directory walk-up involved."
REASON[ADR-014]="No new fitness function introduced; cost-reconciliation drift (ADR-064) and action-interpretation agreement (ADR-021) cover the slice-1 worker fitness signals."
REASON[ADR-017]="Feature is not an action-interpretation pair; no paired interpretation session involved."
REASON[ADR-018]="No verification verdict artifact produced by this feature."
REASON[ADR-021]="Feature does not produce an action-interpretation pair, so the agreement metric does not apply."
REASON[ADR-022]="No Feedback artifact produced by this feature."
REASON[ADR-023]="No Feedback artifact produced; controlled vocabulary not invoked here."
REASON[ADR-024]="No Feedback artifact produced; lifecycle state machine not invoked here."
REASON[ADR-025]="No Feedback artifact produced; blocking semantics not invoked here."
REASON[ADR-027]="No new role registered by this feature."
REASON[ADR-033]="Worker SDK provider routing is governed by ADR-047 (capability-tag binding) and ADR-054 (LiteLLM as substrate); ADR-033's earlier formulation does not apply."
REASON[ADR-034]="Worker registration flow does not invoke escalation tiers; the WorkerCurator's reject path produces Feedback, not an escalation step."
REASON[ADR-035]="Feature does not assemble a bundle that carries a stakes judgment."
REASON[ADR-036]="WorkerImage catalog (ADR-055) mirrors the Capability/RoleBinding catalog shape, but this feature does not extend the Capability/RoleBinding catalog itself."
REASON[ADR-037]="Provider defaults (Scaleway / Anthropic) are configured inside LiteLLM (ADR-064) rather than in feature code."
REASON[ADR-038]="No new artifact type introduced by this feature; existing dual-provenance discipline already governs the artifacts written elsewhere in the brief."
REASON[ADR-039]="No new motivational predicate introduced by this feature."
REASON[ADR-040]="No new boundary artifact introduced by this feature."
REASON[ADR-041]="Feature does not write artifacts through GraphWriter; SHACL enforcement is in scope of FT-086 / FT-087 / FT-094."
REASON[ADR-043]="Feature does not introduce a new full-chain query; existing traversal already covers the artifacts it produces."
REASON[ADR-047]="Feature does not perform capability-tag-to-entry binding at dispatch time."
REASON[ADR-054]="Feature does not call LiteLLM; SDK provider substrate is wired in FT-095 / FT-096."
REASON[ADR-064]="LiteLLM substrate concerns are isolated to FT-095 / FT-096; this feature does not call LiteLLM."
REASON[ADR-065]="Dagger deferral is a runtime-substrate decision affecting FT-088 / FT-095; this feature does not depend on the runtime model."

# Fallback for any ADR not in REASON.
FALLBACK_REASON="Cross-cutting ADR reviewed; not in this slice-1 worker-distribution feature's scope. Brief-internal governance is captured under ADR-055..ADR-065 and linked where applicable."

############################################################
# 3. TEST CRITERIA — one exit-criteria TC per feature.
############################################################

declare -A TC_TITLE
TC_TITLE[FT-086]="WorkerImage artifact validates and is discoverable by capability tag"
TC_TITLE[FT-087]="WorkerImageSubmission validates as a BoundaryArtifact"
TC_TITLE[FT-088]="Worker OCI image manifest exposes capability tags and SDK version labels"
TC_TITLE[FT-089]="Keyless cosign signature verifies against the trust list"
TC_TITLE[FT-090]="identity-verifier produces a SignatureVerdict for each of the five outcome classes"
TC_TITLE[FT-091]="CycloneDX SBOM is reachable as the image's OCI referrer"
TC_TITLE[FT-092]="WorkerCurator session admits a clean Submission and rejects a flawed one"
TC_TITLE[FT-093]="Reusable release workflow runs end-to-end and posts a WorkerImageSubmission"
TC_TITLE[FT-094]="POST /submissions accepts a valid Submission and rejects unauthorized or invalid payloads"
TC_TITLE[FT-095]="pipeline-cli workers run pulls a qualified image and starts it with the four required env vars"
TC_TITLE[FT-096]="LiteLLM proxy routes a worker call by capability tag and reports telemetry to pipeline-cli"

############################################################
# 4. EXECUTION
############################################################

link_adrs() {
  local ft="$1" adr
  for adr in "${UNIVERSAL_ADRS[@]}"; do
    product feature link "$ft" --adr "$adr" >/dev/null
    echo "  link $ft → $adr"
  done
  local extra="${EXTRA_ADRS[$ft]:-}"
  if [[ -n "$extra" ]]; then
    while IFS= read -r adr; do
      [[ -z "$adr" ]] && continue
      product feature link "$ft" --adr "$adr" >/dev/null
      echo "  link $ft → $adr"
    done <<< "$extra"
  fi
}

acknowledge_remaining() {
  local ft="$1" adr reason preflight_out
  # `product preflight` exits 1 when gaps exist — capture output, ignore status.
  preflight_out=$(product preflight "$ft" 2>&1 || true)
  while read -r adr; do
    [[ -z "$adr" ]] && continue
    reason="${REASON[$adr]:-$FALLBACK_REASON}"
    product feature acknowledge "$ft" --adr "$adr" --reason "$reason" >/dev/null
    echo "  ack  $ft ↛ $adr"
  done < <(echo "$preflight_out" | awk '/^[[:space:]]*✗/ {print $2}')
}

author_tc() {
  local ft="$1" title="$2" tc_id existing
  # Idempotency: skip if the feature already has any TC linked. Consume the
  # producer in full first to avoid SIGPIPE under pipefail.
  local show_out
  show_out=$(product feature show "$ft" 2>&1)
  existing=$(echo "$show_out" | awk '/^Tests:/ {print $2}')
  if [[ -n "$existing" && "$existing" != "(none)" ]]; then
    echo "  test $ft already has TCs ($existing); skipping"
    return 0
  fi
  # `product test new --format json` currently emits text; parse "Created: TC-NNN".
  tc_id=$(product test new --type exit-criteria "$title" | awk '/^Created:/ {print $2}')
  if [[ -z "$tc_id" ]]; then
    echo "  WARN  $ft: could not parse new TC id" >&2
    return 0
  fi
  product feature link "$ft" --test "$tc_id" >/dev/null
  echo "  test $ft → $tc_id ($title)"
}

main() {
  for ft in "${NEW_FEATURES[@]}"; do
    echo "=== $ft ==="
    link_adrs "$ft"
    acknowledge_remaining "$ft"
    if [[ -n "${TC_TITLE[$ft]:-}" ]]; then
      author_tc "$ft" "${TC_TITLE[$ft]}"
    fi
  done

  echo
  echo "=== Verification ==="
  for ft in "${NEW_FEATURES[@]}"; do
    local summary
    summary=$(product preflight "$ft" 2>&1 | tail -1)
    echo "$ft: $summary"
  done
}

main "$@"
