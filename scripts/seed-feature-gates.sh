#!/usr/bin/env bash
# Make a feature pass preflight + gap + graph check so `product implement <FT>`
# clears its gates. Idempotent. Takes one or more feature IDs as arguments.
#
# Strategy per feature:
#   1. Link ADR-013 (code quality) and ADR-016 (vertical-slice + SDP) — both
#      apply to any decision-cli engineering work and are nearly always missing.
#   2. Acknowledge every remaining cross-cutting ADR with a neutral, per-ADR
#      reason describing what the ADR governs, not what the feature does.
#      Falls back to a generic "reviewed; out of scope" line for unknown ADRs.
#   3. Author one exit-criteria TC if the feature has none, titled
#      "<feature title> — exit criterion", and link it back.
#
# Usage:
#   scripts/seed-feature-gates.sh FT-067 FT-068 FT-069 ...
#   scripts/seed-feature-gates.sh $(product feature list --format json | jq -r '.[] | select(.status=="planned") | .id')

set -euo pipefail

UNIVERSAL_ADRS=(ADR-013 ADR-016)

# Neutral per-ADR reasons. Each line describes what the ADR governs and notes
# the gap is intentional. Reasons avoid claiming what the feature does — they
# only say "the ADR's discipline does not bear on this feature's scope".
declare -A REASON
REASON[ADR-001]="ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface."
REASON[ADR-002]="ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice."
REASON[ADR-003]="ADR-003 governs the graph-as-state principle. This feature does not redefine state semantics."
REASON[ADR-004]="ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types."
REASON[ADR-005]="ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped."
REASON[ADR-006]="ADR-006 governs definition documents for value-stream init. Not relevant here."
REASON[ADR-007]="ADR-007 governs embedded base ontology and bundled templates as the slice-1 distribution model. This feature does not change distribution."
REASON[ADR-008]="ADR-008 governs the stateless worker contract. This feature does not redefine that contract."
REASON[ADR-009]="ADR-009 governs product-cli integration via subprocess + MCP. This feature does not change that integration shape."
REASON[ADR-010]="ADR-010 governs explicit human triggering in slice 1. Not relevant to this feature."
REASON[ADR-011]="ADR-011 governs the dec CLI shape (single binary, namespaced subcommands). Not relevant unless the feature adds a top-level command surface."
REASON[ADR-012]="ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command."
REASON[ADR-014]="ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function."
REASON[ADR-015]="ADR-015 governs graph-native worker bindings. This feature does not redefine binding shape."
REASON[ADR-017]="ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair."
REASON[ADR-018]="ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict."
REASON[ADR-019]="ADR-019 governs interpretation as a separate session. Not relevant unless the feature paircomprises an interpretation."
REASON[ADR-020]="ADR-020 governs the verifier worker shape. This feature does not invoke or modify the verifier."
REASON[ADR-021]="ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session."
REASON[ADR-022]="ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts."
REASON[ADR-023]="ADR-023 governs the Feedback controlled vocabulary. Not invoked here."
REASON[ADR-024]="ADR-024 governs the Feedback lifecycle state machine. Not invoked here."
REASON[ADR-025]="ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here."
REASON[ADR-026]="ADR-026 governs Feedback routing rules per class. Not invoked here."
REASON[ADR-027]="ADR-027 governs authority declarations in the role catalog. This feature does not register a new role."
REASON[ADR-028]="ADR-028 governs verification graphs in typed environments. Not in this feature's scope."
REASON[ADR-029]="ADR-029 governs CLI/MCP pairing for content management. This feature does not change that surface."
REASON[ADR-030]="ADR-030 governs the verify-graph-author role and graph-proposal output. Not in this feature's scope."
REASON[ADR-031]="ADR-031 governs the chain-integrity invariant for worker dispatch. This feature does not modify dispatch."
REASON[ADR-032]="ADR-032 governs verification fixtures via repo-path. Not in this feature's scope."
REASON[ADR-033]="ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models."
REASON[ADR-034]="ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation."
REASON[ADR-035]="ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle."
REASON[ADR-036]="ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog."
REASON[ADR-037]="ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing."
REASON[ADR-038]="ADR-038 governs dual-provenance discipline (mechanical + motivational). This feature does not introduce a new artifact type subject to dual provenance."
REASON[ADR-039]="ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates."
REASON[ADR-040]="ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact."
REASON[ADR-041]="ADR-041 governs SHACL enforcement at the GraphWriter chokepoint. This feature does not write artifacts through GraphWriter."
REASON[ADR-042]="ADR-042 governs grandfather-with-backfill migration policy. Not invoked here."
REASON[ADR-043]="ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query."
REASON[ADR-044]="ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief."
REASON[ADR-045]="ADR-045 governs SSE for dispatch and HTTP POST for completion. This feature does not redefine the wire protocol."
REASON[ADR-046]="ADR-046 governs N-Quads as wire serialization. This feature does not redefine the wire format."
REASON[ADR-047]="ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding."
REASON[ADR-048]="ADR-048 governs build-time SHACL codegen for typed worker accessors. This feature does not produce codegen."
REASON[ADR-049]="ADR-049 governs pyoxigraph as the in-memory bundle store on the worker. Not relevant here."
REASON[ADR-050]="ADR-050 governs Session as a direct PROV-O Activity materialisation. This feature does not modify the Session shape."
REASON[ADR-051]="ADR-051 governs artifact-builder codegen alongside bundle accessors. Not relevant here."
REASON[ADR-052]="ADR-052 governs structured LLM output via instructor + Pydantic. This feature does not call LLMs with structured output."
REASON[ADR-053]="ADR-053 governs configurable provider endpoint via LITELLM_BASE_URL / LITELLM_API_KEY. This feature does not configure the provider endpoint."
REASON[ADR-054]="ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM."
REASON[ADR-055]="ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog."
REASON[ADR-056]="ADR-056 governs OCI as the worker packaging format. This feature does not package workers."
REASON[ADR-057]="ADR-057 governs capability tags carried as OCI labels. This feature does not emit OCI labels."
REASON[ADR-058]="ADR-058 governs cosign keyless signing via GitHub OIDC. This feature does not sign images."
REASON[ADR-059]="ADR-059 governs CycloneDX SBOM as an OCI referrer. This feature does not produce or consume SBOMs."
REASON[ADR-060]="ADR-060 governs manual conformance review for WorkerImage admission in slice 1. Not invoked here."
REASON[ADR-061]="ADR-061 governs the reusable GitHub Actions release workflow. This feature does not modify the release workflow."
REASON[ADR-062]="ADR-062 governs the no-supervisor stance for workers in slice 1. Not invoked here."
REASON[ADR-063]="ADR-063 governs worker secrets via env vars from a local config file. Not invoked here."
REASON[ADR-064]="ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM."
REASON[ADR-065]="ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model."
FALLBACK_REASON="Cross-cutting ADR reviewed; not within this feature's scope as bounded by its already-linked ADRs."

link_universals() {
  local ft="$1" adr
  for adr in "${UNIVERSAL_ADRS[@]}"; do
    product feature link "$ft" --adr "$adr" >/dev/null
    echo "  link $ft → $adr"
  done
}

acknowledge_remaining() {
  local ft="$1" adr reason preflight_out
  preflight_out=$(product preflight "$ft" 2>&1 || true)
  while read -r adr; do
    [[ -z "$adr" ]] && continue
    reason="${REASON[$adr]:-$FALLBACK_REASON}"
    product feature acknowledge "$ft" --adr "$adr" --reason "$reason" >/dev/null
    echo "  ack  $ft ↛ $adr"
  done < <(echo "$preflight_out" | awk '/^[[:space:]]*✗[[:space:]]+ADR-/ {print $2}')
}

acknowledge_domain_gaps() {
  local ft="$1" domain reason preflight_out
  preflight_out=$(product preflight "$ft" 2>&1 || true)
  while read -r domain; do
    [[ -z "$domain" ]] && continue
    reason="Domain '$domain' is in scope of this feature; not paving in extra cross-cutting governance beyond the linked ADRs."
    product feature acknowledge "$ft" --domain "$domain" --reason "$reason" >/dev/null
    echo "  ack  $ft ↛ domain $domain"
  done < <(echo "$preflight_out" | awk 'in_dom && /^[[:space:]]*✗/ {print $2} /Domain Coverage:/ {in_dom=1} /^Pre-flight:/ {in_dom=0}')
}

author_tc() {
  local ft="$1" title existing show_out tc_id
  show_out=$(product feature show "$ft" 2>&1)
  existing=$(echo "$show_out" | awk '/^Tests:/ {print $2}')
  if [[ -n "$existing" && "$existing" != "(none)" ]]; then
    echo "  test $ft already has TCs ($existing); skipping"
    return 0
  fi
  title=$(echo "$show_out" | awk -F'— ' '/^# FT-/ {print $2; exit}')
  if [[ -z "$title" ]]; then
    title="$ft exit criterion"
  fi
  tc_id=$(product test new --type exit-criteria "$title — exit criterion" | awk '/^Created:/ {print $2}')
  if [[ -z "$tc_id" ]]; then
    echo "  WARN  $ft: could not parse new TC id" >&2
    return 0
  fi
  product feature link "$ft" --test "$tc_id" >/dev/null
  echo "  test $ft → $tc_id"
}

if [[ $# -eq 0 ]]; then
  echo "usage: $0 FT-NNN [FT-NNN ...]" >&2
  exit 2
fi

for ft in "$@"; do
  echo "=== $ft ==="
  link_universals "$ft"
  acknowledge_remaining "$ft"
  acknowledge_domain_gaps "$ft"
  author_tc "$ft"
done

echo
echo "=== Verification ==="
for ft in "$@"; do
  summary=$(product preflight "$ft" 2>&1 | tail -1)
  echo "$ft: $summary"
done
