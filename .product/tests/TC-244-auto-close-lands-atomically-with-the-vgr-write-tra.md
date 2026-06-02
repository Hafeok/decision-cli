---
id: TC-244
title: Auto-close lands atomically with the VGR write transaction
type: invariant
status: passing
validates:
  features:
  - FT-116
  adrs: []
phase: 4
runner: cargo-test
runner-args: features::ft_116_retract_stale_defects::tests::tc_244_auto_close_atomic_with_vgr_write
runner-timeout: 30
observes:
- graph
last-run: 2026-06-01T12:19:42.438075103+00:00
last-run-duration: 0.5s
---

## Description

The VGR commit and the lifecycle transitions must be visible
together or not at all. A reader that sees VGR-N exists but
the prior defects still in `produced` state would dispatch
wrongly; a reader that sees defects `closed` but VGR-N missing
would have unresolved evidence.

## Acceptance Criteria

Cargo test:

1. Seed: graph G, VGR-1 with one defect fb-1 (produced).
2. Inject a fault: SHACL validator rejects the close
   transition (stub that always returns
   `ValidationError::Shape("...")` on `closedByEvidenceRetraction`).
3. Invoke the full VGR-write path
   (`persist_vgr_and_retract(store, VGR-2)`).
4. Assert:
   - The call returns `Err`.
   - VGR-2 is NOT in the store (`SELECT ?vgr WHERE { ?vgr a
     dec:VerificationGraphResult; dec:resultOf <G> } LIMIT
     2` returns only VGR-1).
   - fb-1's lifecycle is still `produced` (close didn't
     land).
   - The store's full quad count is byte-identical to its
     pre-call state.
5. Remove the SHACL fault; re-run. Assert both VGR-2 and the
   fb-1 close land together in one snapshot.