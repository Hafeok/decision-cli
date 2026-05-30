---
id: TC-235
title: Orphan worktrees from harness crash are pruned at next start
type: scenario
status: failing
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
last-run: 2026-05-30T12:06:40.684205971+00:00
failure-message: "warning: field `tc_iri` is never read\n  --> crates/decision-cli/src/features/ft_116_retract_stale_defects/query.rs:15:9\n   |\n11 | pub struct StaleDefect {\n   |            ----------- field in this struct\n...\n15 |     pub tc_iri: String,\n   |         ^^^^^^\n   |\n   = note: `StaleDefect` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: function `han"
last-run-duration: 1.2s
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