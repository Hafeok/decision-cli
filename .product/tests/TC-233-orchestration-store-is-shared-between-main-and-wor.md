---
id: TC-233
title: Orchestration store is shared between main and worktree via absolute path
type: invariant
status: passing
validates:
  features:
  - FT-115
  adrs: []
phase: 4
runner: cargo-test
runner-args: tc_233_store_shared_between_main_and_worktree
runner-timeout: 120
observes:
- file
- graph
last-run: 2026-05-30T12:17:03.882540613+00:00
last-run-duration: 0.5s
---

## Description

Worktrees isolate *source*, not *lifecycle state*. The
orchestration store must remain single-instance in main so
two parallel sessions see the same lifecycle truth (open
defects, supersededBy edges, dispatch records). Splitting the
store per worktree would split-brain feedback addressing.

## Acceptance Criteria

Cargo test:

1. Compose main repo with `.dec/store/orchestration.nq`
   containing a marker triple
   `<urn:marker:tc-233> <urn:p> "1" .`.
2. Create a worktree for sess-abc.
3. Assert `.dec/worktrees/sess-abc/.dec/` does NOT exist as a
   real directory — it's either absent or a symlink/junction
   pointing to main's `.dec/`.
4. From the worktree's tree-of-record, open the orchestration
   store at its absolute path (i.e., main's
   `.dec/store/orchestration.nq`). Read the marker triple.
   Assert it's present.
5. Write a new triple from the worker's context (worker
   simulates a `dec session record` call). Re-read from main's
   context. Assert the new triple is visible — both readers
   see the same backing store.
6. Abort the worktree. Assert the marker triple AND the
   worker-written triple both still exist (they live in
   main's store, not the worktree).