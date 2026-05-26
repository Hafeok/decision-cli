---
id: TC-183
title: Defect-feedback bundle field is populated when feedback exists for the (feature, env) pair
type: exit-criteria
status: passing
validates:
  features:
  - FT-107
  adrs:
  - ADR-026
  - ADR-066
phase: 3
runner: cargo-test
runner-args: tc_183_defect_feedback_bundle_field_is_populated
runner-timeout: 60
last-run: 2026-05-26T18:54:31.667893283+00:00
last-run-duration: 0.7s
---

## Claim

When the orchestration store contains one or more `dec:Feedback` artifacts with `class = "defect"`, `targetRole = "verifier"`, and `lifecycleState = "produced"` whose addressing step belongs to a graph that `dec:verifies <feature_iri>` in environment `<env_iri>`, the bundle assembled for `dec verify graph generate <feature_id> --environment <env_id>` carries a non-empty `defect_feedback` array on the `VerifyGraphAuthorInputJson` envelope.

## Scenarios

### Setup

- A fresh `dec init`-bootstrapped tree.
- One feature_spec (`FT-T1`) with one TC.
- One ephemeral env (`ENV-T1`).
- One `dec:VerificationGraph` (`VG-T1`) verifying `FT-T1` in `ENV-T1` with one shell step.
- Two `dec:Feedback` artifacts (`FB-1`, `FB-2`), both `class=defect targetRole=verifier lifecycleState=produced`, both with `dec:addressingArtifact` pointing at `VG-T1`'s step.

### Test

Call `verify_graph_generate::run_generate` for `(FT-T1, ENV-T1)` with mode `PrintOnly` to avoid persistence. Capture the assembled bundle. Assert:

1. `bundle.defect_feedback.len() == 2`.
2. Both entries have `class = "defect"`, the right `feedback_iri`s, and the right `graph_id` (`VG-T1`).
3. Entries are sorted by `emitted_at` descending (most recent first).

### Negative paths

- A `dec:Feedback` with `class=gap` is NOT included in the array.
- A `dec:Feedback` with `targetRole=spec-author` is NOT included.
- A `dec:Feedback` with `lifecycleState=addressed` is NOT included.
- A `dec:Feedback` whose addressing step belongs to a graph verifying a different feature is NOT included.