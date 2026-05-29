---
id: TC-233
title: Orchestration store is shared between main and worktree via absolute path
type: invariant
status: unimplemented
validates:
  features:
  - FT-115
  adrs: []
observes:
- file
- graph
phase: 4
runner: cargo-test
runner-args: tc_233_orchestration_store_shared
runner-timeout: 60
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
