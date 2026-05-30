---
id: TC-229
title: Worktree is created at dispatch start branched off feature baseline commit
type: scenario
status: passing
validates:
  features:
  - FT-115
  adrs: []
phase: 4
runner: cargo-test
runner-args: tc_229_worktree_created_at_baseline
runner-timeout: 120
observes:
- file
- graph
last-run: 2026-05-30T12:17:03.882540613+00:00
last-run-duration: 0.6s
---

## Description

Worktree creation is the load-bearing first step. Without it,
the worker has no isolated workspace and the whole feature
degenerates back to today's in-place shape. The branch must
point at the captured baseline so the worker's diff is
attributable to *this* dispatch, not to whatever else happened
in main while the worker ran.

## Acceptance Criteria

Cargo test:

1. Compose a temp git repo with an initial commit at `baseline_sha`.
2. Call `create_worktree(workdir, "sess-abc", baseline_sha)`.
3. Assert:
   - `.dec/worktrees/sess-abc/` exists and is a valid git
     checkout (`git -C <path> rev-parse HEAD` returns
     `baseline_sha`).
   - A branch `dec/sess/sess-abc` exists pointing at the same
     SHA (`git branch --list "dec/sess/sess-abc"`).
   - The orchestration store carries
     `<session> dec:worktreePath <abs_path>` and
     `<session> dec:worktreeBaseline <baseline_sha>` quads.
4. Make a commit in main (advance HEAD past `baseline_sha`).
   Assert the worktree branch's tip is unchanged — the
   worktree branched off the *captured* baseline, not the
   live HEAD.