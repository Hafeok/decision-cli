---
id: TC-231
title: Successful dispatch fast-forwards worktree commit into main and deletes worktree
type: scenario
status: passing
validates:
  features:
  - FT-115
  adrs: []
phase: 4
runner: cargo-test
runner-args: tc_231_successful_merge_and_cleanup
runner-timeout: 120
observes:
- file
- graph
last-run: 2026-05-30T12:17:03.882540613+00:00
last-run-duration: 0.6s
---

## Description

The happy path: worker commits inside the worktree, harness
fast-forwards main, deletes the worktree. After this sequence
main's HEAD carries the worker's commit, the worktree is
gone, and the session is recorded as merged.

## Acceptance Criteria

Cargo test:

1. Compose main repo at `baseline_sha`. Create worktree for
   sess-abc.
2. In the worktree, write `src/lib.rs = "fixed"` and commit
   `[FT-X] fix lib` (simulating worker). Capture the
   worktree-tip SHA as `worker_sha`.
3. Call `fast_forward_into_main(workdir, worktree_path)`.
   Assert returns `MergeOutcome::FastForwarded { from:
   baseline_sha, to: worker_sha }`.
4. Assert:
   - `git -C main rev-parse HEAD` returns `worker_sha`.
   - `main/src/lib.rs` contains `"fixed"` (worker's edit is
     now visible in main).
   - The orchestration store carries
     `<session> dec:mergedInto <worker_sha>` and
     `<session> dec:worktreeStatus "merged"`.
5. Call `abort_worktree(workdir, worktree_path)`. Assert:
   - `.dec/worktrees/sess-abc/` no longer exists.
   - The branch `dec/sess/sess-abc` no longer exists.
   - main's HEAD is unchanged (still `worker_sha`).