---
id: TC-248
title: Post-migration dec verify bench list returns the migrated entries with BNCH ids
type: scenario
status: unimplemented
validates:
  features:
  - FT-117
  adrs: []
observes:
- stdout
phase: 4
runner: bash
runner-args: tests/scripts/tc-248-post-migration-bench-list.sh
runner-timeout: 60
---

## Description

End-to-end roundtrip: migrate a workdir with 3 pre-rename
ENV entries, then run `dec verify bench list` and confirm
all 3 appear with BNCH-prefixed ids. Catches the silent-
incomplete-migration failure mode (instance IRIs got
rewritten but rdf:type didn't, so `bench list` showed empty
even though the store had quads with `bench/` IRIs).

## Acceptance Criteria

Bash test:

1. Compose a temp workdir with `.dec/store/orchestration.nq`
   pre-populated with 3 `VerificationEnvironment` instances:
   `ENV-001` (ephemeral-tempdir), `ENV-002` (ephemeral-
   tempdir), `ENV-100` (some other type). Use full
   pre-rename vocabulary (`dec:envType`,
   `dec:VerificationEnvironment` class).
2. Pre-check: `dec verify bench list` returns "no benches
   yet" (the pre-migration baseline). Assert this — confirms
   the bug condition.
3. Run `dec _migrate-env-to-bench --workdir <temp>`. Assert
   exit 0.
4. Run `dec verify bench list`. Assert stdout contains all
   three rows with the renamed ids:
   - `BNCH-001` with type `ephemeral-tempdir`.
   - `BNCH-002` with type `ephemeral-tempdir`.
   - `BNCH-100` with its original type.
5. Assert no row mentions `ENV-001`, `ENV-002`, or `ENV-100`
   (the old ids should not appear).
