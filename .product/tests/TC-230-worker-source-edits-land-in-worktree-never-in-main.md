---
id: TC-230
title: Worker source edits land in worktree, never in main pre-merge
type: invariant
status: unimplemented
validates:
  features:
  - FT-115
  adrs: []
observes:
- file
phase: 4
runner: cargo-test
runner-args: tc_230_worker_edits_isolated_to_worktree
runner-timeout: 60
---

## Description

The core isolation guarantee: while the worker runs, every
file edit appears in the worktree's filesystem path, none in
main's. Without this property, two parallel implementer
dispatches can never run safely, and the "abort discards
worker state" guarantee leaks.

## Acceptance Criteria

Cargo test:

1. Compose a temp main repo with file `src/lib.rs` containing
   `"original"`. Commit. Capture `main_root`.
2. Create a worktree for session sess-abc.
3. Stub a worker function that:
   - Reads `worker_workdir/src/lib.rs`, asserts it sees `"original"`.
   - Writes `worker_workdir/src/lib.rs` with `"edited"`.
   - Creates a new file `worker_workdir/src/new_helper.rs`.
   - Does NOT commit.
4. After the worker returns (no commit, no merge yet), assert:
   - `main_root/src/lib.rs` still contains `"original"` (worker
     edits did not leak into main).
   - `main_root/src/new_helper.rs` does not exist.
   - `worktree/src/lib.rs` contains `"edited"`.
   - `worktree/src/new_helper.rs` exists.
5. Run `git status` in main; assert clean working tree.
6. Run `git status` in the worktree; assert two modified/new
   files exactly.
