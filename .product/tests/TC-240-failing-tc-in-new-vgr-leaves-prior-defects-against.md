---
id: TC-240
title: Failing TC in new VGR leaves prior defects against that TC unchanged
type: invariant
status: passing
validates:
  features:
  - FT-116
  adrs: []
phase: 4
runner: cargo-test
runner-args: features::ft_116_retract_stale_defects::tests::tc_240_failing_tc_leaves_prior_defects_unchanged
runner-timeout: 30
observes:
- graph
last-run: 2026-05-30T11:20:24.875643969+00:00
last-run-duration: 0.5s
---

## Description

Only flipped-to-pass TCs trigger auto-close. If a TC stays
failing (or transitions fail→fail), the prior defect is still
valid — closing it would lose real signal. Mixed VGRs (some
pass, some fail) must close only the passing subset.

## Acceptance Criteria

Cargo test:

1. Seed: graph G, VGR-1 with two defects: fb-A (TC T-A, fail)
   and fb-B (TC T-B, fail).
2. Write VGR-2 for G with `outcome="pass"` for T-A but
   `outcome="fail"` for T-B.
3. Invoke `retract_stale_defects(store, VGR-2)`.
4. Assert:
   - fb-A is `closed` with the retraction citation.
   - fb-B is still `produced` (unchanged) — its TC is still
     failing.
   - The store has NOT emitted a new defect for T-B as part of
     the auto-close pass (defect emission is the verifier's
     job, not the auto-close job).