---
id: TC-232
title: Failed dispatch deletes worktree and leaves main byte-identical to pre-dispatch
type: invariant
status: failing
validates:
  features:
  - FT-115
  adrs: []
phase: 4
runner: cargo-test
runner-args: tc_232_failed_dispatch_cleanup
runner-timeout: 120
observes:
- file
- graph
last-run: 2026-05-30T12:06:40.684205971+00:00
failure-message: "warning: field `tc_iri` is never read\n  --> crates/decision-cli/src/features/ft_116_retract_stale_defects/query.rs:15:9\n   |\n11 | pub struct StaleDefect {\n   |            ----------- field in this struct\n...\n15 |     pub tc_iri: String,\n   |         ^^^^^^\n   |\n   = note: `StaleDefect` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: function `han"
last-run-duration: 1.2s
---

## Description

The pollution-prevention invariant. This is the property the
FT-115 motivation cited: a worker that produces 895 file
edits and exits without committing must leave main as if
nothing happened. Tested across three failure modes that
together cover the observed real-world failures.

## Acceptance Criteria

Cargo test, three cases:

1. **Worker produces no commit.** Compose main at `baseline_sha`,
   take a byte-snapshot of every tracked file. Create worktree.
   Stub worker that edits files in the worktree but exits
   without committing. Harness detects no commit on the
   worktree branch, calls abort. Assert:
   - main's HEAD is `baseline_sha`.
   - Every file in main is byte-identical to its snapshot.
   - The orchestration store records
     `<session> dec:worktreeStatus "aborted"` with reason
     `"no commit"`.
2. **Worker commits to wrong branch.** Same setup; worker
   commits to `main` (or any branch other than
   `dec/sess/sess-abc`). Harness detects the contract
   violation, aborts. Assert main is byte-identical to
   snapshot.
3. **Scope-guard rejects worker commit.** Same setup; worker
   commits to its assigned branch but the commit touches files
   outside the feature's scope. Scope-guard fires, harness
   aborts. Assert main is byte-identical to snapshot, and the
   abort reason names the offending files.

After every case, `.dec/worktrees/sess-abc/` does not exist.