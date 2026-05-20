---
id: TC-047
title: dec doctor exit code mirrors worker resolution outcome
type: exit-criteria
status: unimplemented
validates:
  features:
  - FT-016
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-047-dec-doctor-exit-code.sh
runner-timeout: 60
---

## Description

`dec doctor` is the operator-facing audit that re-runs worker preflight on demand. Unlike `dec init`, it is **authoritative**: its exit code must reflect whether every required worker resolved. It is read-only — no writes to the store, no writes to the working tree, no real subprocess invocations.

## Acceptance Criteria

Given an already-initialised `.dec/store/orchestration.nq` and the bundled `engineering-development` value stream:

1. **All resolved → exit 0.** With `code-writer` resolvable, `dec doctor` exits **0** and stdout contains exactly one `code-writer  OK  <path>` row.

2. **Any missing → non-zero.** With `code-writer` unresolvable, `dec doctor` exits **non-zero** (the same status `2` `dec init` uses), and stdout contains the `MISSING` row plus install hints.

3. **`--role <role>` filters.** `dec doctor --role code-writer` returns only the row(s) for that role; an unknown role argument exits non-zero with a clear error.

4. **Read-only — store.** A sha256 of `.dec/store/orchestration.nq` taken before and after the run is identical. The store file mtime is unchanged.

5. **Read-only — workspace.** `git status --porcelain` produces identical output before and after the run.

6. **Probes are side-effect-free.** No descendant process of `dec doctor` invokes the resolved worker binary with stdin data; probing uses `which` and `python3 -c "import <module>"` only. Verifiable by `strace -f` or by tracing `execve` calls in the test harness.

## Fixture

- Reuse the TC-046 tempdir post-bootstrap.
- Resolvable / unresolvable passes achieved as in TC-046.

## Out of scope

- The JSON output shape (TC-048).
- The shared resolution-chain implementation (TC-050).
