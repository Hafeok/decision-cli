---
id: TC-213
title: dec verify bench list returns post-migration BNCH-NNN ids identical to pre-migration ENV-NNN
type: scenario
status: unimplemented
validates:
  features:
  - FT-112
  adrs: []
observes:
- stdout
phase: 4
runner: bash
runner-args: tests/scripts/tc-213-bench-list-roundtrip.sh
runner-timeout: 60
---

## Description

End-to-end roundtrip: a workspace seeded with two ENV instances,
migrated, and queried via `dec verify bench list` returns the
same suffixes as the original ENV-NNN ids — just with the
`BNCH-` prefix. Verifies that the rename did not lose, alias,
or duplicate any verification-bench record.

## Acceptance Criteria

Bash test:

1. Compose a temp `.dec/store/orchestration.nq` containing two
   verification-environment instances: `ENV-001` and `ENV-002`,
   each with `dec:envType "local"` and a description literal.
2. Capture `dec verify env list` (using the *pre-migration*
   binary path — or simulate by writing the same Turtle that
   the env list command would parse). Record the two ids and
   their types/descriptions.
3. Run `dec _migrate-env-to-bench`. Assert exit 0.
4. Run `dec verify bench list`. Assert stdout contains:
   - One row with id `BNCH-001`, type `local`, identical
     description.
   - One row with id `BNCH-002`, type `local`, identical
     description.
   - No rows mentioning `ENV-001` or `ENV-002` (the old IDs
     are gone).

Cardinality preservation is the load-bearing assertion: pre-
migration count equals post-migration count.
