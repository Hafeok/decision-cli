---
id: TC-232
title: Failed dispatch deletes worktree and leaves main byte-identical to pre-dispatch
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
runner-args: tc_232_failed_dispatch_leaves_main_untouched
runner-timeout: 60
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
