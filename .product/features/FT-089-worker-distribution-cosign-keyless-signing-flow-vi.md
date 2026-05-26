---
id: FT-089
title: 'worker-distribution: Cosign keyless signing flow via GitHub OIDC'
phase: 3
status: complete
depends-on: []
adrs:
- ADR-058
- ADR-013
- ADR-016
- ADR-044
tests:
- TC-131
domains: []
domains-acknowledged:
  ADR-035: Feature does not assemble a bundle that carries a stakes judgment.
  ADR-065: Dagger deferral is a runtime-substrate decision affecting FT-088 / FT-095; this feature does not depend on the runtime model.
  ADR-025: No Feedback artifact produced; blocking semantics not invoked here.
  ADR-043: Feature does not introduce a new full-chain query; existing traversal already covers the artifacts it produces.
  ADR-054: Feature does not call LiteLLM; SDK provider substrate is wired in FT-095 / FT-096.
  ADR-024: No Feedback artifact produced; lifecycle state machine not invoked here.
  ADR-034: Worker registration flow does not invoke escalation tiers; the WorkerCurator's reject path produces Feedback, not an escalation step.
  ADR-047: Feature does not perform capability-tag-to-entry binding at dispatch time.
  ADR-017: Feature is not an action-interpretation pair; no paired interpretation session involved.
  ADR-004: Feature does not emit dispatch or session events; PROV-O hookup happens in features that write artifacts.
  ADR-014: No new fitness function introduced; cost-reconciliation drift (ADR-064) and action-interpretation agreement (ADR-021) cover the slice-1 worker fitness signals.
  ADR-018: No verification verdict artifact produced by this feature.
  ADR-039: No new motivational predicate introduced by this feature.
  ADR-041: Feature does not write artifacts through GraphWriter; SHACL enforcement is in scope of FT-086 / FT-087 / FT-094.
  ADR-022: No Feedback artifact produced by this feature.
  ADR-021: Feature does not produce an action-interpretation pair, so the agreement metric does not apply.
  ADR-012: Not a per-stream command; no working-directory walk-up involved.
  ADR-023: No Feedback artifact produced; controlled vocabulary not invoked here.
  ADR-055: Cross-cutting ADR reviewed; not in this slice-1 worker-distribution feature's scope. Brief-internal governance is captured under ADR-055..ADR-065 and linked where applicable.
  ADR-038: No new artifact type introduced by this feature; existing dual-provenance discipline already governs the artifacts written elsewhere in the brief.
  ADR-001: Application-layer feature; does not touch the oxi-events crate boundary.
  ADR-036: WorkerImage catalog (ADR-055) mirrors the Capability/RoleBinding catalog shape, but this feature does not extend the Capability/RoleBinding catalog itself.
  ADR-002: Feature ships infrastructure / packaging conventions, not graph mutations.
  ADR-027: No new role registered by this feature.
  ADR-064: LiteLLM substrate concerns are isolated to FT-095 / FT-096; this feature does not call LiteLLM.
  ADR-005: Worker-registration discipline is independent of value-stream scope.
  ADR-037: Provider defaults (Scaleway / Anthropic) are configured inside LiteLLM (ADR-064) rather than in feature code.
  ADR-040: No new boundary artifact introduced by this feature.
  ADR-033: Worker SDK provider routing is governed by ADR-047 (capability-tag binding) and ADR-054 (LiteLLM as substrate); ADR-033's earlier formulation does not apply.
---

## Motivation

Derived from `brief:worker-distribution-slice-1`. Catalog admission requires a verifiable signing identity per WorkerImage. Addresses ADR-058 (cosign keyless via GitHub OIDC).

## Scope

- Worker release workflow uses `cosign sign --keyless` with the ambient GitHub OIDC token. No private key material in repos.
- The signing identity (Fulcio-issued certificate subject and issuer) is captured on the `WorkerImageSubmission` (`signature_identity` field) so admission can verify.
- The Rekor transparency log entry produced by cosign is referenced from the Submission so the verifier (FT-090) can confirm both signature validity and log inclusion.
- A trust list of permitted Fulcio identities is maintained inside the orchestration system, matched by:
  - GitHub repo (owner/name)
  - Workflow path (`.github/workflows/release.yml` or a specific reusable workflow ref)
  - Tag pattern (e.g. `implementer-v*.*.*`)
  - Only signatures whose identity matches a listed entry are valid.
- Local key-based signing remains supported as a fallback (development workflows outside GitHub Actions). A local-key identity is admissible only if explicitly enrolled in the trust list.

## Out of scope

- Identity rotation policy when a signing identity is compromised (deferred; tracked in brief open questions).
- Cross-CI portability (slice 3+ concern; the wire-level primitives — OIDC, cosign, Rekor — are portable, only the workflow file is GitHub-specific).
- Automatic trust-list management (slice 1: edited manually by the operator).

## Success criteria

- A release workflow signs an image keyless and the resulting Submission carries Fulcio identity + Rekor entry pointers that the verifier resolves and accepts.
- A signature from an identity NOT on the trust list is rejected at verification time with verdict `untrusted-identity`.
- A submission claiming a Rekor entry that doesn't exist (or doesn't match) is rejected with verdict `rekor-entry-missing`.
