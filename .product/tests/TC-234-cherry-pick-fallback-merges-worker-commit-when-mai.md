---
id: TC-234
title: Cherry-pick fallback merges worker commit when main moved during dispatch
type: scenario
status: failing
validates:
  features:
  - FT-115
  adrs: []
phase: 4
runner: cargo-test
runner-args: tc_234_cherry_pick_when_main_moves
runner-timeout: 120
observes:
- file
- exit-code
last-run: 2026-05-30T12:06:40.684205971+00:00
failure-message: "warning: field `tc_iri` is never read\n  --> crates/decision-cli/src/features/ft_116_retract_stale_defects/query.rs:15:9\n   |\n11 | pub struct StaleDefect {\n   |            ----------- field in this struct\n...\n15 |     pub tc_iri: String,\n   |         ^^^^^^\n   |\n   = note: `StaleDefect` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: function `han"
last-run-duration: 1.3s
---

## Description

The race: dispatch start captures `baseline_sha`, worker runs
for minutes, in the meantime a concurrent dispatch (or
operator) advances main to `concurrent_sha`. Fast-forward
fails. The harness must fall back to cherry-pick so the
worker's diff still lands.

## Acceptance Criteria

Cargo test:

1. Main at `baseline_sha`, file `src/a.rs = "A"`. Create
   worktree branched from `baseline_sha`.
2. In the worktree, edit `src/a.rs = "A-fixed"`, commit.
   Capture `worker_sha`.
3. In main (simulating a concurrent dispatch), edit
   `src/b.rs = "B"`, commit. Capture `concurrent_sha`. Note
   `b.rs` is disjoint from the worker's diff.
4. Call `fast_forward_into_main(workdir, worktree_path)`.
5. Assert returns `MergeOutcome::CherryPicked { onto:
   concurrent_sha, picked_sha: <new_sha> }`. (Fast-forward
   would have failed; cherry-pick succeeded.)
6. Assert main's HEAD is the new cherry-picked SHA (not
   `worker_sha` directly, since cherry-pick creates a new
   commit).
7. Assert main carries both edits: `src/a.rs = "A-fixed"` AND
   `src/b.rs = "B"`.
8. **Conflict case**: repeat with main editing `src/a.rs =
   "A-different"`. Cherry-pick will conflict. Assert returns
   `Err(MergeError::Conflict { paths: ["src/a.rs"] })` and
   main's HEAD is unchanged (`concurrent_sha`, no half-merged
   state).