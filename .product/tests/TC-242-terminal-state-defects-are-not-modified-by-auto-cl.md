---
id: TC-242
title: Terminal-state defects are not modified by auto-close pass
type: invariant
status: passing
validates:
  features:
  - FT-116
  adrs: []
phase: 4
runner: cargo-test
runner-args: features::ft_116_retract_stale_defects::tests::tc_242_terminal_state_defects_not_modified
runner-timeout: 30
observes:
- graph
last-run: 2026-06-01T12:19:42.438075103+00:00
last-run-duration: 0.6s
---

## Description

ADR-024 says lifecycle states `closed`, `rejected`, and
`superseded` are terminal — no further transitions. Auto-close
must respect this; rewriting a `closed` defect to `closed`
again (or worse, `closed` over a `rejected`) corrupts the audit
trail and breaks the monotonicity invariant.

## Acceptance Criteria

Cargo test:

1. Seed three defects from prior VGR-1 of graph G against TC T:
   - fb-X with lifecycle `closed` (worker already addressed it)
   - fb-Y with lifecycle `rejected` (operator manually rejected)
   - fb-Z with lifecycle `superseded` (got superseded by
     another feedback)
2. Write VGR-2 for G with `outcome="pass"` for T.
3. Invoke `retract_stale_defects(store, VGR-2)`.
4. Assert:
   - fb-X's lifecycle is still `closed`, with no new
     `closedByEvidenceRetraction` triple (the original closer
     stays authoritative).
   - fb-Y's lifecycle is still `rejected`.
   - fb-Z's lifecycle is still `superseded`.
   - The pre/post quad count for the three terminal feedbacks
     is unchanged.