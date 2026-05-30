---
id: TC-236
title: Two concurrent implementer dispatches into separate worktrees do not interfere with each other
type: scenario
status: failing
validates:
  features:
  - FT-115
  adrs: []
phase: 4
runner: cargo-test
runner-args: tc_236_concurrent_worktrees_no_interference
runner-timeout: 120
observes:
- file
- graph
last-run: 2026-05-30T12:06:40.684205971+00:00
failure-message: "warning: field `tc_iri` is never read\n  --> crates/decision-cli/src/features/ft_116_retract_stale_defects/query.rs:15:9\n   |\n11 | pub struct StaleDefect {\n   |            ----------- field in this struct\n...\n15 |     pub tc_iri: String,\n   |         ^^^^^^\n   |\n   = note: `StaleDefect` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: function `han"
last-run-duration: 1.2s
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