---
id: TC-231
title: Successful dispatch fast-forwards worktree commit into main and deletes worktree
type: scenario
status: failing
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
last-run: 2026-05-30T12:06:40.684205971+00:00
failure-message: "warning: field `tc_iri` is never read\n  --> crates/decision-cli/src/features/ft_116_retract_stale_defects/query.rs:15:9\n   |\n11 | pub struct StaleDefect {\n   |            ----------- field in this struct\n...\n15 |     pub tc_iri: String,\n   |         ^^^^^^\n   |\n   = note: `StaleDefect` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: function `han"
last-run-duration: 1.1s
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