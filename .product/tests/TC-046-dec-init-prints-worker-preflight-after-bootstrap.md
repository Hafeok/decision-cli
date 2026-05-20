---
id: TC-046
title: dec init prints worker preflight after bootstrap
type: exit-criteria
status: passing
validates:
  features:
  - FT-016
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-046-dec-init-preflight.sh
runner-timeout: 60
last-run: 2026-05-20T08:03:50.124890079+00:00
last-run-duration: 0.2s
---

## Description

After `dec init --from <stream>.ttl` finishes seeding the orchestration store, it must run the worker preflight audit and surface the result inline. The audit is advisory at init time — a fresh host with no worker installed must still get a working `.dec/store/orchestration.nq`, just with a distinct exit code and clear install hints.

## Acceptance Criteria

Given a tempdir containing the bundled `engineering-development` value stream:

1. **Workers present.** With `code-writer` resolvable (either on `$PATH` or via `CODE_WRITER_CMD`), `dec init --from <stream>.ttl` exits **0**, stdout contains a `Worker preflight:` header followed by a row matching `^\s*code-writer\s+OK\s+\S+`, and `.dec/store/orchestration.nq` exists with the bootstrap session.

2. **Workers missing.** With `$PATH` scrubbed of `code-writer` and `CODE_WRITER_CMD` unset, `dec init --from <stream>.ttl` exits with the **advisory status 2**, stdout contains a `MISSING` row for `code-writer`, and stdout contains an indented `To install:` block mentioning `uv tool install ./workers/code-writer` and the `CODE_WRITER_CMD` override.

3. **Bootstrap is not rolled back by audit failure.** In case (2), `.dec/store/orchestration.nq` still exists and contains the bootstrap session (`prov:Activity` of type `dec:BootstrapSession`).

4. **Inactive roles render with a dash.** Any role declared in the manifest but not referenced by the current value stream renders as a `—` row with `role not active in current value stream`.

## Fixture

- Tempdir with `git init`, an unscoped value stream TTL.
- Two test passes: one with `code-writer` resolvable, one with it unresolvable (achieved by overriding `PATH` to a known-empty directory and clearing `CODE_WRITER_CMD`).
- The manifest sha256 captured in the bootstrap session telemetry is asserted to match `dec`'s embedded value.

## Out of scope

- The audit's behaviour on `dec doctor` invocation (covered by TC-047).
- JSON output shape (covered by TC-048).
- `dec implement` abort behaviour on missing workers (covered by TC-049).