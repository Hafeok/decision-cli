---
id: FT-115
title: 'decision-cli: implementer dispatch runs in a per-session git worktree'
phase: 4
status: planned
depends-on: []
adrs: []
tests:
- TC-229
- TC-230
- TC-231
- TC-232
- TC-233
- TC-234
- TC-235
- TC-236
domains:
- api
- storage
domains-acknowledged:
  storage: Manages .dec/worktrees/ as ephemeral source-tree copies; orchestration store stays single-instance in main and is shared by absolute path. Worker binary install loop is the only shared-resource contention.
  api: Adds worktree lifecycle and the bundle field rename (repo_root → worker_workdir); no new operator-facing CLI surface beyond the hidden _worktree list/prune diagnostics.
---

## Description

Today the implementer worker runs in-place against the main
working tree. Three real failure modes follow:

1. **Pollution from incomplete dispatches.** The implementer
   can produce hundreds of file edits and exit without
   committing (worker error, scope-guard rejection, operator
   Ctrl-C, OOM, etc.). The main checkout is left with
   uncommitted changes that have to be manually staged or
   reverted. Just-witnessed with FT-112: an in-progress run
   left 895 uncommitted files in the tree.
2. **Sequential-only sweep.** `dec drive ship --all` runs
   feature-by-feature because two implementer dispatches into
   the same working tree would step on each other (one
   worker's edits land in the other's bundle, both commits
   conflict). Parallelism is forced to nil.
3. **No clean abort.** "Throw this dispatch away" requires
   the operator to read the diff and decide what to revert.
   There's no `dec drive abort sess:abc` that atomically
   undoes a single worker's contribution.

This feature changes the implementer dispatch shape so each
session runs inside its own ephemeral git worktree at
`.dec/worktrees/<short-session-id>/`. The harness creates the
worktree at dispatch start, the worker writes and commits
inside it, the harness either fast-forwards the commit into
main on success or deletes the worktree on failure. The main
checkout is touched at most once per dispatch — at the moment
the harness chooses to land the work — and operators get a
clean atomic boundary between "worker's intermediate state"
and "main repo state."

A first-class consequence: **multiple implementer dispatches
can run in parallel**. Each gets its own worktree, edits a
disjoint slice of files (per feature), and lands its commit
into main when ready. FT-111's `dec drive ship --all` becomes
free to dispatch N implementers concurrently up to a
configured cap, dramatically shortening the wall-clock for
multi-feature sweeps. GraphWriter coordination is still
required (the orchestration store stays single-writer), but
source-tree contention — the harder of the two problems —
is resolved by construction.

A secondary benefit: the scope-guard heuristic
(`finalize_run` only-edit-files-in-feature-history check) we
landed last week becomes redundant. The worktree merge is the
strong guarantee: only the worker's actual diff lands; any
files the worker touched outside its intended scope get
filtered out by the cherry-pick filter. Scope-guard goes from
"heuristic that blocks bad dispatches" to "harness invariant
that bad dispatches cannot mutate main."

## Functional Specification

### Inputs

No new CLI flags for the operator-facing surface — `dec drive
ship FT-XXX` works identically from the operator's
perspective. The harness manages the worktree lifecycle
internally.

Harness-level configuration (read from value-stream
capability bindings, or hardcoded sensible defaults for
slice-1):

- Worktree root: `.dec/worktrees/` (under workdir).
- Worktree branch name: `dec/sess/<short-session-id>` so
  `git branch` lists them readably.
- Maximum concurrent implementer dispatches: `1` for slice-1
  (matches today's sequential shape), with the eventual goal
  of N (operator-configurable). The parallel-dispatch path
  ships in this feature; the sweep's adoption of it is a
  follow-up.

For debugging:

- `dec _worktree list` — hidden CLI listing live worktrees,
  their session IRIs, baseline commits, and elapsed times.
  Useful when a worktree leaks (harness crash mid-dispatch).
- `dec _worktree prune` — hidden CLI deleting orphaned
  worktrees (worktrees with no live session in the
  orchestration store).

### Outputs

**New module** —
`crates/decision-cli/src/features/ft_115_implementer_worktree/`:

- `create.rs` — `create_worktree(workdir, session_short_id,
  baseline_commit) -> Result<WorktreePath, WorktreeError>`.
  Wraps `git worktree add .dec/worktrees/<sid>
  <baseline_commit>` plus the branch naming.
- `merge.rs` — `fast_forward_into_main(workdir, worktree_path)
  -> Result<MergeOutcome, MergeError>`. Fast-forwards main to
  the worktree branch's tip; falls back to cherry-pick if a
  concurrent main commit happened between dispatch start and
  merge.
- `abort.rs` — `abort_worktree(workdir, worktree_path) ->
  Result<(), WorktreeError>`. Runs `git worktree remove
  --force <worktree>` and `git branch -D
  dec/sess/<sid>`. Idempotent.
- `coordinate.rs` — the GraphWriter / `.dec/` coordination
  layer (see Behaviour §State sharing).
- `cli.rs` — adapter wiring for the `_worktree list/prune`
  diagnostics.
- `tests.rs` — unit + integration tests per the TC list.

**Harness extension** — the existing implementer dispatch
path (`features/implement/lifecycle.rs::finalize_implement_run`
and its callers) gains worktree create/merge/abort calls
around the worker invocation. The worker's bundle no longer
contains `repo_root` pointing at main; it contains
`worktree_root` pointing at the per-session worktree path.

**Bundle change**:

- `repo_root: PathBuf` field → renamed to `worker_workdir:
  PathBuf`, the worker's tree-of-record for the dispatch.
- Worker prompt teaching extended: "Your workdir is a fresh
  git worktree branched off the feature baseline. Edit
  freely; commit when done with `git -C $WORKER_WORKDIR
  commit -m '[FT-XXX] …'`. The harness will land your
  commit into main on success or discard it on failure."

### State

The orchestration store (`.dec/store/orchestration.nq`)
stays **single instance in main** and is shared across all
worktrees. Worktrees access it by absolute path. No
per-worktree store; that would split-brain the lifecycle
state.

Worktrees themselves live under `.dec/worktrees/` so
`.gitignore` cleanly excludes them (under the existing
`.dec/` ignore rule if you add one, or a new `.dec/worktrees/`
rule). The worktrees are ephemeral; no production worktree
should survive past its session's terminal state.

Per-dispatch state (the worktree path, baseline commit, status)
is recorded in the orchestration store as a new triple
attached to the Session activity. This way `dec session show
<id>` can report whether a session's worktree is live, merged,
or aborted.

### Behaviour

1. **Dispatch start.** Before invoking the worker, the
   harness:
   a. Captures `baseline = git rev-parse HEAD` in the main
      workdir.
   b. Calls `create_worktree(workdir,
      session_short_id, baseline)`. This runs
      `git worktree add .dec/worktrees/<sid> <baseline>` —
      checkout, fresh branch `dec/sess/<sid>` pointing at
      baseline.
   c. Records `<session> dec:worktreePath <abs_path>` and
      `<session> dec:worktreeBaseline <commit>` in the store.
2. **Worker invocation.** The bundle's `worker_workdir`
   field is the worktree path, not main. The worker's
   prompt teaches it to edit and commit inside that path. The
   worker does NOT touch main directly; the harness blocks
   any worker that tries (a verifier check at finalize
   time).
3. **Cargo install / uv install coordination.** When the
   worker needs to install a freshly-edited binary so the
   verifier picks it up, the worker runs `cargo install
   --path "$WORKER_WORKDIR/crates/decision-cli" --bin dec
   --offline`. This installs into the user's shared
   `~/.cargo/bin/`. The install is the source of contention
   — if two worktrees install simultaneously, the last one
   wins, breaking the first. Slice-1 caps concurrent
   implementer dispatches to 1 (see Inputs); the multi-
   concurrent path is a downstream feature that introduces
   a binary install lease (lockfile under `.dec/locks/`).
4. **Finalize on worker success.** When the worker exits
   with a commit on the worktree branch:
   a. Harness reads the worktree branch's tip.
   b. Runs the scope-guard check (currently
      `finalize_run` defect-scoped path) as a *belt-and-
      braces* sanity check. The worktree is the strong
      guarantee; scope-guard remains as a redundant
      lightweight diff filter (and a future-friendly hook
      where SHACL or other gates could attach).
   c. Calls `fast_forward_into_main(workdir,
      worktree_path)`. Tries fast-forward first; if main
      moved between dispatch start and merge (another
      session committed something), falls back to
      cherry-pick.
   d. Records `<session> dec:mergedInto <commit_sha>` and
      `<session> dec:worktreeStatus "merged"` in the store.
   e. Calls `abort_worktree(...)` to remove the worktree
      directory and branch.
5. **Finalize on worker failure.** Worker errored, no commit
   on the worktree branch, or scope-guard rejected the
   commit:
   a. Harness records `<session> dec:worktreeStatus
      "aborted"` with the reason.
   b. Calls `abort_worktree(...)`.
   c. Main is byte-identical to its pre-dispatch state.
6. **Crash recovery.** On harness startup, scan
   `.dec/worktrees/` for directories without a live session
   reference in the store. For each orphan, log + delete via
   `abort_worktree`. The diagnostic `dec _worktree prune`
   exposes this as an operator action too.
7. **Verifier dispatches are unaffected.** Verifier reads
   from main's `.dec/verify/` and `git log` to ground its
   evidence. The merge in step 4c is the moment the
   verifier's view changes; no verifier dispatch operates on
   an unmerged worktree.

### Invariants

- The main working tree is byte-identical before and after an
  aborted implementer dispatch (modulo `.dec/store/...` which
  is the lifecycle-state surface).
- The main working tree changes exactly once per *successful*
  implementer dispatch — at the fast-forward / cherry-pick
  moment. Workers cannot interleave half-formed states into
  main.
- Worktrees are ephemeral. Every worktree directory in
  `.dec/worktrees/` corresponds to a live (non-terminal)
  session, OR will be pruned at the next harness start.
- Worktree branches (`dec/sess/<sid>`) are named so a `git
  branch | grep dec/sess/` query enumerates them; operators
  can clean up manually if needed.
- The orchestration store is single-writer (lives in main);
  worktrees access it by absolute path.
- Worker binaries are installed into `~/.cargo/bin/`, shared
  across worktrees. Slice-1 serialises implementer
  dispatches to avoid install-race; concurrent dispatches
  are a follow-up.
- The scope-guard check still runs at finalize, redundantly,
  as a safety net. The worktree shape doesn't remove it —
  it makes it cheaper (it sees only the worktree's tip
  commit) and turns it into a verifier of a stronger
  invariant.

### Error handling

- `git worktree add` fails (uncommitted changes in main,
  missing branch, disk full, etc.): harness fails the
  dispatch with a clear error, records the failure on the
  session, leaves main untouched.
- `git worktree remove --force` fails (stuck process holding
  a file open): harness logs the error and continues; the
  next `_worktree prune` pass will retry. Lingering
  worktrees are a nuisance, not a correctness issue.
- Fast-forward fails because main moved (race with another
  successful merge): harness falls back to cherry-pick. If
  cherry-pick fails too (conflict), records the conflict on
  the session and aborts; operator resolves manually via
  `git -C <worktree> rebase main` then `dec _worktree merge
  <sid>`.
- Worker exits without committing on the worktree branch:
  treat as failure (no merge), abort the worktree.
- Worker commits to a branch other than the assigned one:
  treat as failure (worker contract violation), abort the
  worktree.
- Worker touches files outside its assigned worktree (e.g.
  edits main directly via absolute paths): finalize detects
  via `git diff --quiet` on main pre/post worker and aborts
  with a contract-violation error.

### Boundaries

- This feature does NOT introduce parallel implementer
  dispatch in this slice. Slice-1 ships the worktree
  shape with concurrency cap = 1. Lifting the cap is a
  separate feature (it requires the binary install lease
  and the cherry-pick conflict-resolution path).
- This feature does NOT change the verify-graph-author or
  verifier dispatch shapes. Those still operate against
  main directly; they're read-only relative to source.
- This feature does NOT touch the worker SDK contract beyond
  the bundle field rename (`repo_root` → `worker_workdir`).
  Workers continue to receive a path and edit it; the path
  just refers to a worktree now.
- This feature does NOT manage operator-authored work in
  the main tree. If the operator has uncommitted edits in
  main, the dispatch start fails fast — the operator must
  commit or stash before dispatching. (Today's behaviour
  silently mixes operator and worker edits; this is
  intentionally stricter.)
- This feature does NOT introduce a long-running worktree
  service. Worktrees live for the duration of one dispatch;
  the harness creates and destroys them per-call.

## Out of scope

- **Parallel implementer dispatch with N > 1.** Belongs to a
  follow-up that introduces the binary install lease,
  per-worktree cargo target dir (avoid recompiling shared
  deps N times), and the GraphWriter contention path.
  Without those, raising the cap risks the failure modes
  this feature was designed to prevent.
- **Verifier worktrees.** Verifier dispatches don't write
  source, so per-dispatch worktrees don't add isolation —
  they only add overhead. Verifiers stay in main.
- **Pushing the worktree branch to a remote** as part of the
  merge. Worktree branches are local-only; the harness
  fast-forwards into main locally and deletes the branch.
  Pushing main happens at the operator's pace.
- **Conflict-resolution UI.** If cherry-pick fails, the
  operator resolves manually. A future feature could add
  `dec _worktree merge --resolve` with an interactive
  picker, but slice-1 just records the conflict and aborts.
- **A different baseline strategy than HEAD.** Worktrees
  branch off `HEAD` at dispatch start. Operators who want
  to dispatch against a non-HEAD baseline (e.g. test a
  patch against a release branch) get that via a future
  flag; today's dispatch is always against HEAD.
- **Removing the scope-guard.** It stays as a redundant
  safety net. Removing it is a separate decision that needs
  evidence the worktree guarantee is sufficient on its own.
