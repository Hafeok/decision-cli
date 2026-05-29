---
id: TC-236
title: Two concurrent implementer dispatches into separate worktrees do not interfere with each other
type: scenario
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
runner-args: tc_236_two_concurrent_dispatches_do_not_interfere
runner-timeout: 60
---

## Description

The parallel-dispatch payoff: the source-tree contention that
forces today's sweep to be sequential is resolved by
construction once each implementer has its own worktree. This
TC proves the property at the layer FT-115 ships (the
worktree shape itself); the configured concurrency cap of 1
means slice-1 doesn't expose this externally yet, but the
shape is verifiably parallelism-safe.

## Acceptance Criteria

Cargo test (uses `tokio::test` with `tokio::join!` for
deterministic concurrency):

1. Compose main repo at `baseline_sha` with files
   `src/feature_a.rs` and `src/feature_b.rs`. Capture a
   byte-snapshot of every tracked file.
2. Spawn two parallel implementer dispatches:
   - **Worker A** writes to `worktree_a/src/feature_a.rs` and
     commits `[FT-A] add A`.
   - **Worker B** writes to `worktree_b/src/feature_b.rs` and
     commits `[FT-B] add B`.
3. `tokio::join!` both. Assert:
   - Both workers complete successfully (neither's edits leak
     into the other's worktree).
   - `worktree_a/src/feature_a.rs` was modified;
     `worktree_a/src/feature_b.rs` matches the snapshot.
   - `worktree_b/src/feature_b.rs` was modified;
     `worktree_b/src/feature_a.rs` matches the snapshot.
   - Each worktree's git log shows exactly its own commit.
4. Merge both serially (one at a time). Assert main's HEAD
   carries both commits (one as fast-forward, the second as
   either fast-forward or cherry-pick depending on order).
5. Assert main's tree carries BOTH workers' edits — feature_a
   and feature_b are both modified.
6. Both sessions in the orchestration store carry
   `dec:worktreeStatus "merged"`.

This proves the worktree shape supports N=2 parallel dispatches
even though slice-1's harness caps at 1; lifting the cap is a
follow-up feature.
