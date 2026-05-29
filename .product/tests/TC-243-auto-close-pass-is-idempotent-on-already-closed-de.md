---
id: TC-243
title: Auto-close pass is idempotent on already-closed defects
type: invariant
status: unimplemented
validates:
  features:
  - FT-116
  adrs: []
observes:
- graph
phase: 4
runner: cargo-test
runner-args: tc_243_auto_close_is_idempotent
runner-timeout: 30
---

## Description

Operators may invoke `dec _retract-stale-defects --graph
VG-NNN` multiple times for the same graph (script automation,
recovering from a crash mid-transition). Re-running on an
already-processed VGR must produce no observable diff. Without
this, repeated invocations would emit duplicate
`closedByEvidenceRetraction` triples and stale `closedAt`
timestamps drifting forward.

## Acceptance Criteria

Cargo test:

1. Seed: graph G, VGR-1 with one defect fb-1 (produced).
2. Write approved VGR-2 for G.
3. Invoke `retract_stale_defects(store, VGR-2)` — first call
   closes fb-1.
4. Snapshot the store's quads relevant to fb-1.
5. Invoke `retract_stale_defects(store, VGR-2)` again.
6. Assert:
   - Returns `Ok(0_closed)` on the second call (clear signal
     to operators that nothing happened).
   - fb-1's quads are byte-identical pre/post the second
     call (including `closedAt` — no timestamp drift).
   - No duplicate `dec:closedByEvidenceRetraction` quad
     emitted.
