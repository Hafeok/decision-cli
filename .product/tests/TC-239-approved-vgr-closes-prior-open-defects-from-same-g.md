---
id: TC-239
title: Approved VGR closes prior open defects from same graph against same passing TC
type: scenario
status: unimplemented
validates:
  features:
  - FT-116
  adrs: []
observes:
- graph
phase: 4
runner: cargo-test
runner-args: tc_239_approved_vgr_closes_prior_defects
runner-timeout: 60
---

## Description

The happy path that motivated FT-116: a prior failing VGR for
graph G emitted a defect for TC T; a fresh VGR for the same
G reports T as passing; the prior defect transitions to
`closed` automatically. Without this transition the planner
keeps the defect open and dispatches phantom work.

## Acceptance Criteria

Cargo test:

1. Seed an in-memory store with:
   - Graph G (`VG-100`) verifying feature F.
   - VGR-1 (`VGR-500`) for G with `outcome="fail"` projecting
     evidence for TC T (`TC-200`) → emitted defect fb-1
     (`fb:abc`) with lifecycle `produced`, source_artifact=T,
     source_session=VGR-1.
2. Write a new approved VGR-2 (`VGR-501`) for G with
   `outcome="pass"` projecting evidence for T.
3. Invoke `retract_stale_defects(store, VGR-501)`.
4. Assert:
   - fb-1's `dec:lifecycleState` is now `"closed"`.
   - fb-1 has a `dec:closedByEvidenceRetraction <VGR-501>`
     triple.
   - fb-1 has a `dec:closedAt` timestamp.
   - fb-1's source_artifact and source_session are unchanged
     (the close is non-destructive of identity fields).
   - The store contains exactly one new lifecycle quad per
     transition; no duplicate writes.
