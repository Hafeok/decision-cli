---
id: TC-235
title: Orphan worktrees from harness crash are pruned at next start
type: scenario
status: passing
validates:
  features:
  - FT-115
  adrs: []
phase: 4
runner: cargo-test
runner-args: tc_235_orphan_pruning
runner-timeout: 120
observes:
- file
last-run: 2026-05-30T12:17:03.882540613+00:00
last-run-duration: 0.5s
---

## Description

A harness crash mid-dispatch can leave a worktree on disk
without a live session record. Subsequent dispatches must
detect and clean these orphans so `.dec/worktrees/` doesn't
grow without bound. Also exposed as `dec _worktree prune` for
operator-driven cleanup.

## Acceptance Criteria

Cargo test:

1. Compose main repo. Create three worktrees: sess-alpha,
   sess-beta, sess-gamma.
2. Record sess-alpha and sess-gamma in the orchestration store
   with `dec:worktreeStatus "live"`. sess-beta has NO record
   (simulates harness crash before the record-write).
3. Call `prune_orphans(workdir)`. Assert returns
   `PruneOutcome { pruned: ["sess-beta"], kept: ["sess-alpha",
   "sess-gamma"] }`.
4. Assert:
   - `.dec/worktrees/sess-beta/` does NOT exist.
   - Branch `dec/sess/sess-beta` does NOT exist.
   - `.dec/worktrees/sess-alpha/` and
     `.dec/worktrees/sess-gamma/` still exist (their sessions
     are live).
5. Mark sess-alpha as `dec:worktreeStatus "merged"`. Call
   prune again. Assert sess-alpha is now pruned (terminal
   sessions whose worktrees survived are also orphans).
6. **Stuck-process case**: hold a file handle open inside
   sess-gamma's worktree. Call prune. Assert prune logs the
   failure for sess-gamma but doesn't panic, and returns
   `kept: ["sess-gamma"]`. Next call after the handle
   releases prunes it.