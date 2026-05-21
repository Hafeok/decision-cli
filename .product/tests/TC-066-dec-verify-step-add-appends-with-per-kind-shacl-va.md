---
id: TC-066
title: dec verify step add appends with per-kind SHACL validation
type: exit-criteria
status: unimplemented
validates:
  features: []
  adrs: []
phase: 2
---

## Description

[FT-044](FT-044)'s exit criterion: `dec verify step add` appends a typed step to an existing graph, validates per-kind fields against [FT-036](FT-036)'s SHACL, and atomically rewrites the on-disk Turtle.

## Acceptance Criteria

1. **Append shell-command.** Given an empty `VG-001`, `dec verify step add VG-001 --type shell-command --field command="dec init" --field expect-exit-code=0` appends one step at position 1, exits 0, prints the minted step IRI and position. The on-disk `.ttl` now carries the step in `dec:steps`.

2. **Append sparql-assertion.** A second invocation `--type sparql-assertion --field target=.dec/store --field query="..." --field expect-rows=1` appends at position 2. The order across positions 1 and 2 matches authoring order in both the on-disk Turtle and a subsequent `dec verify graph show`.

3. **Per-kind SHACL.** `--type shell-command` without `--field command=...` fails with `Error::SchemaViolation`; the detail names `command`. Same pattern for each of the 6 seed kinds (missing required field → SchemaViolation with that field named).

4. **Unknown step type.** `--type rocketship` exits 2 with `Error::InvalidArgument { field: "step_type" }`.

5. **Graph not found.** `dec verify step add VG-999 --type shell-command ...` exits 1 with `Error::ArtifactNotFound`.

6. **Atomic file rewrite.** A simulated I/O failure during the file rewrite (e.g. write-then-rename interrupted) leaves the previous Turtle intact — no partial write, no corruption. Verified by killing the process mid-rewrite in a fault-injection test.

7. **MCP parity.** `dec_verify_step_add` with equivalent JSON input produces the same `dec:steps` list as the CLI invocation.

8. **`${name}` accepted literally.** `--field command="dec verify ${prior_capture}"` succeeds; the literal `${prior_capture}` string is preserved in the on-disk Turtle.

## Fixture

- Tempdir with `dec init`, a graph `VG-001` from FT-041, the seeded `ENV-001-ephemeral-cli`.

## Out of scope

- Safety enforcement integration (TC-067).
- Step removal / reorder.
