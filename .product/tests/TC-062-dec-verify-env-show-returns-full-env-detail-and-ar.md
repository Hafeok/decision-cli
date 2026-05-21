---
id: TC-062
title: dec verify env show returns full env detail and ArtifactNotFound on unknown id
type: exit-criteria
status: unimplemented
validates:
  features: []
  adrs: []
phase: 2
---

## Description

[FT-040](FT-040)'s `dec verify env show <ENV-NNN>` and `dec_verify_env_show` return a single env's full document. Unknown ids surface as `ArtifactNotFound`.

## Acceptance Criteria

1. **Show seeded env.** `dec verify env show ENV-001-ephemeral-cli` after `dec init` returns a multi-line render containing the id, env-type, safety-class, all allowed ops, setup, teardown, and on-disk path.

2. **JSON format.** `--format json` emits a single JSON object with every property of the env document; missing optional fields are omitted (not `null`).

3. **Round-trip.** Reserialising the JSON output back to Turtle yields canonically equal Turtle to the on-disk file.

4. **MCP parity.** `dec_verify_env_show` with `{ id, format: "json" }` returns the same JSON object as the CLI `--format json` invocation.

5. **Unknown id.** `dec verify env show ENV-999` exits 1 with `Error::ArtifactNotFound { kind: "VerificationEnvironment", id: "ENV-999" }`; stderr names the kind and id. MCP returns the structured error.

6. **Malformed id.** `dec verify env show not-an-id` exits 2 with `Error::InvalidArgument { field: "id" }`.

## Fixture

- Tempdir with `dec init` plus one additional env authored via FT-038.

## Out of scope

- History / change log (slice 3+).
- Show by alias.
