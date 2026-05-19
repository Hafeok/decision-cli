---
id: FT-017
title: 'decision-cli: Implementer finalizes its run (commit + status transition)'
phase: 1
status: planned
depends-on:
- FT-011
adrs:
- ADR-009
- ADR-010
- ADR-011
tests:
- TC-018
domains: []
domains-acknowledged:
  ADR-004: FT-017 produces no new Session/Event/CodeChange artifact; PROV-O links are emitted in the commit message body as plain references rather than as new graph quads.
  ADR-002: FT-017 writes to git and shells out to product-cli; the orchestration store is already finalized by FT-011 step 6, so no graph-as-state mutation happens here.
  ADR-014: FT-017 is a behavior-level feature; it does not author or modify any cross-cutting rule. Compliance is verified by the existing CI gates.
  ADR-013: FT-017 lands new functions in implement.rs (already at 916 lines). Implementation will extract a finalize submodule to keep the host file under the 400-line hard limit.
  ADR-001: FT-017 is harness-side post-run automation in the decision-cli crate; it never touches the oxi-events SDP boundary.
---

## Description

FT-011 wires the implementer dispatch end-to-end up to the point a `CodeChange` is registered, but stops short of the two steps a human operator immediately performs by hand afterwards:

1. Commit the working-tree changes the worker produced.
2. Transition the feature_spec status from `in-progress` to `complete`.

The first run of `dec implement FT-015` exposed the gap: the worker wrote files, the Session was marked `complete` in the orchestration store, but `git status` was still dirty and the feature_spec stayed at `in-progress`. The operator had to finish the job manually — at which point the orchestration record stopped reflecting reality.

This feature closes both gaps so a successful `dec implement` run leaves the working tree clean and the feature_spec at the right status, without the operator needing to remember the follow-up steps.

## Functional Specification

### Inputs

- A successful `ImplementOutcome` (FT-011): Session IRI, Dispatch IRI, CodeChange IRI, feature id, and the worker's summary text.
- The current git working tree (assumed to be at the decision-cli repository root; ADR-012 leaves multi-repo orchestration out of slice 1).
- A reachable `product` binary on `$PATH` (ADR-009).

### Outputs

- A new git commit on `HEAD` whose subject line is `[FT-XXX] <one-line summary>` and whose body includes the Session IRI and bundle hash for cross-graph traceability.
- The feature_spec status flipped to `complete` via `product feature status FT-XXX complete`.
- A telemetry block in `dec implement`'s stdout summary noting the commit SHA and the post-transition status.

### State

- No new persistent state. The commit is recorded in git; the status transition is recorded in `.product/requests.jsonl` by product-cli.

### Behaviour

1. After the harness writes the `CodeChange` to the product-cli graph slice and marks the Session `complete` (FT-011 step 6), it shells out to `git status --porcelain` to determine whether the worker actually produced uncommitted changes.
2. If the working tree is clean, skip the commit step entirely and log `no working-tree changes — skipping commit` to stdout. The status transition still runs (a no-op write-through is harmless).
3. If the working tree is dirty, run `git add -A` followed by `git commit -m <message>`. The message follows the convention from `CLAUDE.md`:
    ```
    [FT-XXX] <first non-blank line of worker summary, truncated to 72 chars>

    Session:     <session-iri>
    Dispatch:    <dispatch-iri>
    CodeChange:  <code-change-iri>
    Bundle:      sha256:<bundle-hash>
    ```
    The body keeps the cross-graph PROV-O reference reachable from `git log` without requiring product-cli to be on the reader's machine.
4. Resolve the commit SHA from `git rev-parse HEAD` and include it in `ImplementOutcome`.
5. Invoke `product feature status FT-XXX complete --root <product-root>` as subprocess. The `--root` is the same root resolved by FT-011 step 5.
6. Print a closing telemetry line:
    ```
    Commit:    <short-sha>
    Status:    FT-XXX → complete
    ```

### Invariants

- The git commit happens **after** the `CodeChange` is persisted and the Session is marked `complete`. A failure to commit must not invalidate the orchestration record — the operator can always re-run `git commit` by hand.
- The feature_spec status transition is the **last** observable side-effect. A failure here surfaces a clear error so the operator can run `product feature status` manually.
- The harness never bypasses git hooks. `--no-verify` is forbidden (CLAUDE.md "Git Safety Protocol").
- A clean working tree is not an error — the worker may have produced a CodeChange whose effects were already committed (e.g. a re-run after a partial implementation), or a stub run with no real writes.

### Error handling

- `git` not on `$PATH` → warn on stdout, skip the commit step, continue to status transition. The Session record still reflects success.
- `git add` / `git commit` non-zero exit → return a `FinalizeError::CommitFailed` carrying stderr; the harness propagates the error but does **not** retry. The CodeChange and Session records are already durable.
- `product feature status` non-zero exit (or `product` not on `$PATH`) → warn on stdout, do not fail the overall run. The next `dec implement` cycle (or a human) can retry the transition.

### Boundaries

- Does NOT push to a remote. Push is a separate operator decision (CLAUDE.md "Git Safety Protocol").
- Does NOT amend or rebase existing commits. Each `dec implement` produces at most one new commit.
- Does NOT run `product verify --platform` — that is FT-014 / FT-015's mechanical-check pipeline, already wired into CI.
- Does NOT close pull requests or transition any other artifact's status (CodeChange status is owned by the worker; Session status is owned by the harness).

## Out of scope

- Squashing multiple `dec implement` runs into a single commit.
- Branching strategy (per-feature branches, etc.) — slice 1 commits to whatever branch the operator is on.
- Operator-facing UI to amend the commit message before the commit lands.
- Signing commits (`-S`) — relies on user/global git config, not the harness.
