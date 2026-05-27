---
id: TC-188
title: DispatchPayloadJson defect_feedback is populated when implementer-targeted feedback exists for the feature
type: exit-criteria
status: passing
validates:
  features:
  - FT-108
  adrs:
  - ADR-026
phase: 3
runner: cargo-test
runner-args: tc_188_dispatch_payload_carries_defect_feedback
runner-timeout: 60
last-run: 2026-05-27T09:13:24.586155434+00:00
last-run-duration: 0.7s
---

## Claim

When `features::implement::lifecycle::build_dispatch_payload` assembles the payload for a feature whose TCs are addressed by one or more `class=defect` / `targetRole=implementer` / `lifecycleState=produced` feedback artifacts, the resulting `DispatchPayloadJson.defect_feedback` array carries those entries. Feedback whose `targetRole != "implementer"` or whose `sourceArtifact` is outside the feature's TC set is NOT included.

## Scenarios

### Setup

- A feature `FT-T188` with TCs `[TC-T188a, TC-T188b]`.
- Three feedback artifacts in the store:
  - `FB-1`: class=defect, targetRole=implementer, source_artifact=TC-T188a → INCLUDED.
  - `FB-2`: class=defect, targetRole=implementer, source_artifact=TC-T188b → INCLUDED.
  - `FB-3`: class=defect, targetRole=verifier, source_artifact=TC-T188a → EXCLUDED (wrong role).
  - `FB-4`: class=defect, targetRole=implementer, source_artifact=TC-other → EXCLUDED (out of feature scope).

### Test

Call `build_dispatch_payload(workdir, "FT-T188", ...)`. Assert:

1. `payload.defect_feedback.len() == 2`.
2. The two records are FB-1 and FB-2 (by feedback_iri).
3. The bundle_hash differs from a sibling call that returns an empty `defect_feedback` (the hash recomputes over the enriched payload).