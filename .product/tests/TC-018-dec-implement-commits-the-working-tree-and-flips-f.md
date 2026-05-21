---
id: TC-018
title: dec implement commits the working tree and flips feature status
type: exit-criteria
status: passing
validates:
  features:
  - FT-017
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test tc_018_finalize_commit_and_status
runner-timeout: 120
last-run: 2026-05-20T08:13:41.742381560+00:00
last-run-duration: 1.0s
---

## Description

`dec implement FT-XXX` must finish its run with the working tree committed and the target feature_spec at status `complete` — otherwise the orchestration record diverges from the on-disk reality.

## Acceptance Criteria

Given a git-initialized fixture repo with `.dec/` and `.product/` seeded, where the code-writer worker produces at least one tracked file change in stub mode:

1. After `dec implement FT-XXX` exits 0, `git status --porcelain` returns empty (no uncommitted changes).
2. `git log -1 --format=%s` matches the regex `^\[FT-XXX\] `.
3. `git log -1 --format=%B` contains the substrings `Session:`, `Dispatch:`, `CodeChange:`, and `Bundle: sha256:`.
4. The new commit was produced **after** the orchestration store was persisted: the Session IRI cited in the commit body resolves in `.dec/store/orchestration.nq` with `dec:status "complete"`.
5. After the run, `product feature show FT-XXX --format json` reports `"status": "complete"`.
6. Hooks were not bypassed: `git log -1 --format=%H` produces a commit whose message body does not contain `--no-verify` and a deliberately failing pre-commit hook causes the run to surface a `FinalizeError::CommitFailed`.

## Fixture

- A throwaway git repo created in `tempdir()` with an initial commit (so `HEAD` exists).
- A minimal `.product/` containing FT-XXX at status `in-progress`.
- `CODE_WRITER_STUB=1` so the worker is deterministic.

## Out of scope

- Push behavior (per ADR boundary in FT-017).
- Commit signing (`-S`) — relies on global git config.
- Multi-commit batching across multiple `dec implement` runs.