---
id: FT-090
title: 'decision-cli: identity-verification action and signature-validity verdict'
phase: 3
status: planned
depends-on:
- FT-089
adrs:
- ADR-013
- ADR-016
- ADR-044
- ADR-017
- ADR-018
- ADR-021
- ADR-038
- ADR-039
- ADR-041
tests:
- TC-132
domains: []
domains-acknowledged:
  ADR-014: No new fitness function introduced; cost-reconciliation drift (ADR-064) and action-interpretation agreement (ADR-021) cover the slice-1 worker fitness signals.
  ADR-024: No Feedback artifact produced; lifecycle state machine not invoked here.
  ADR-043: Feature does not introduce a new full-chain query; existing traversal already covers the artifacts it produces.
  ADR-001: Application-layer feature; does not touch the oxi-events crate boundary.
  ADR-004: Feature does not emit dispatch or session events; PROV-O hookup happens in features that write artifacts.
  ADR-025: No Feedback artifact produced; blocking semantics not invoked here.
  ADR-036: WorkerImage catalog (ADR-055) mirrors the Capability/RoleBinding catalog shape, but this feature does not extend the Capability/RoleBinding catalog itself.
  ADR-040: No new boundary artifact introduced by this feature.
  ADR-012: Not a per-stream command; no working-directory walk-up involved.
  ADR-064: LiteLLM substrate concerns are isolated to FT-095 / FT-096; this feature does not call LiteLLM.
  ADR-047: Feature does not perform capability-tag-to-entry binding at dispatch time.
  ADR-027: No new role registered by this feature.
  ADR-037: Provider defaults (Scaleway / Anthropic) are configured inside LiteLLM (ADR-064) rather than in feature code.
  ADR-035: Feature does not assemble a bundle that carries a stakes judgment.
  ADR-023: No Feedback artifact produced; controlled vocabulary not invoked here.
  ADR-022: No Feedback artifact produced by this feature.
  ADR-002: Feature ships infrastructure / packaging conventions, not graph mutations.
  ADR-055: Cross-cutting ADR reviewed; not in this slice-1 worker-distribution feature's scope. Brief-internal governance is captured under ADR-055..ADR-065 and linked where applicable.
  ADR-054: Feature does not call LiteLLM; SDK provider substrate is wired in FT-095 / FT-096.
  ADR-033: Worker SDK provider routing is governed by ADR-047 (capability-tag binding) and ADR-054 (LiteLLM as substrate); ADR-033's earlier formulation does not apply.
  ADR-034: Worker registration flow does not invoke escalation tiers; the WorkerCurator's reject path produces Feedback, not an escalation step.
  ADR-005: Worker-registration discipline is independent of value-stream scope.
  ADR-065: Dagger deferral is a runtime-substrate decision affecting FT-088 / FT-095; this feature does not depend on the runtime model.
---

## Motivation

Derived from `brief:worker-distribution-slice-1`. The WorkerCurator's bundle needs a structured verdict on whether a candidate image's signature is valid AND whether the signing identity is on the trust list. That verdict is the output of a pure-execution + interpretation action pair, matching the action-interpretation pairing requirement (ADR-017).

## Scope

- Action role: `identity-verifier`. Flavor: `pure_execution + interpretation`.
- Action mechanics (pure_execution side):
  - Runs `cosign verify` against the candidate image's signature using the trust list.
  - Validates that the Rekor entry referenced on the Submission exists and matches.
- Interpretation (separate session, paired with the action per ADR-019):
  - Produces a `SignatureVerdict` artifact with one of these classes:
    - `valid` — signature checks, identity on trust list, Rekor inclusion confirmed.
    - `invalid-signature` — cosign verify failed cryptographically.
    - `untrusted-identity` — signature valid but signer not on trust list.
    - `image-not-found` — registry returned 404 for the candidate ref.
    - `rekor-entry-missing` — referenced Rekor entry doesn't exist or doesn't match.
- The verdict artifact feeds the WorkerCurator's bundle (FT-092). A `valid` verdict is required (but not sufficient) for admission.

## Out of scope

- Conformance audit (separate decision in FT-092 / ADR-060).
- SBOM scanning (slice 3+).
- Identity rotation (deferred).

## Success criteria

- For each of the five verdict classes, an end-to-end test produces the expected verdict given the corresponding input conditions.
- The verdict artifact has mechanical provenance pointing to the action session, and motivational provenance pointing to the originating `WorkerImageSubmission`.
- The Curator's bundle (FT-092) includes the verdict; the absence of a verdict blocks bundle assembly.
