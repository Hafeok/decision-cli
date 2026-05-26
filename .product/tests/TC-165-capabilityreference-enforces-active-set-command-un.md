---
id: TC-165
title: CapabilityReference enforces active-set command uniqueness; supersession is the only path to evolve
type: scenario
status: unimplemented
validates:
  features:
  - FT-101
  adrs: []
phase: 1
---

## Claim

`dec catalog capability new` enforces uniqueness of `dec:command` **across the non-superseded set only** — a second `new` for the same command name is rejected, but the operator can `supersede` the existing reference and then author a new active one for the same command.

## Scenarios

### Setup

- Fresh `.dec/` initialised via `dec init`.
- Empty `.dec/catalog/capabilities/`.

### Scenario A — first author succeeds

Invoke `dec catalog capability new CR-001 --command "dec verify graph new" --version 0.3.0 --from-file fixtures/cr-001-body.json`. Assertions:

- Exit code: 0.
- File `.dec/catalog/capabilities/CR-001.ttl` exists and parses as a SHACL-valid `dec:CapabilityReference` with `dec:command = "dec verify graph new"` and `dec:capabilityVersion = "0.3.0"`.

### Scenario B — duplicate active command is rejected

Without superseding CR-001, invoke `dec catalog capability new CR-002 --command "dec verify graph new" --version 0.3.1 --from-file fixtures/cr-002-body.json`. Assertions:

- Exit code: 1.
- Stderr contains `DuplicateActive` and references `CR-001` as the conflicting existing reference.
- File `.dec/catalog/capabilities/CR-002.ttl` is **not** created.

### Scenario C — supersession unblocks a fresh active author

Invoke `dec catalog capability supersede CR-001 --by CR-002 --new-file fixtures/cr-002-body.json --new-version 0.3.1`. (One verb that writes both the new reference and the supersession edge in a single transaction; the test must assert atomicity — either both writes happen or neither.) Assertions:

- Exit code: 0.
- `CR-001` is on disk with a `dec:supersededBy <CR-002>` predicate.
- `CR-002` is on disk with `dec:supersedes <CR-001>` and `dec:command = "dec verify graph new"`.
- `dec catalog capability list --active` returns CR-002 only; CR-001 is not in the active set.
- `dec catalog capability list --include-superseded` returns both.

### Scenario D — cycle in supersession is rejected at write time

Attempt to write `CR-001 dec:supersedes CR-002` (after Scenario C). Expected: `Error::SupersessionCycle`, exit 1, write rejected by SHACL at the StreamWriter chokepoint.

## Runner

`bash tests/scripts/tc-165-capability-uniqueness.sh`. Temp `.dec/` per the established pattern (TC-158 style). Fixtures `cr-001-body.json` / `cr-002-body.json` ship alongside the script with the minimal valid CapabilityReference body shape.

## Non-goals

- Validation of the `dec:capabilityBody` JSON schema (a separate concern; the fixture body is assumed valid).
- Cross-stream uniqueness (one stream per catalog in v1).
- Bundle assembler reading the references (that's TC-168 under FT-102).
