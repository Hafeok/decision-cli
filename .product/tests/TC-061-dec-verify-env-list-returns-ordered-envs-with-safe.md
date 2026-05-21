---
id: TC-061
title: dec verify env list returns ordered envs with safety-class and type filters
type: exit-criteria
status: unimplemented
validates:
  features: []
  adrs: []
phase: 2
---

## Description

[FT-039](FT-039)'s `dec verify env list` and `dec_verify_env_list` return envs in ascending `ENV-NNN` order, with optional filtering by `--safety-class` and `--type`. Both `--format table` and `--format json` are supported.

## Acceptance Criteria

1. **Empty store.** With only the seeded `ephemeral-cli` env present, `dec verify env list` prints a single-row table; `--format json` returns a one-element array.

2. **Order.** With envs `ENV-001`, `ENV-002`, `ENV-003` present, the listing returns them in ascending order regardless of file mtime or insertion order.

3. **Filter by safety class.** Given mixed envs, `dec verify env list --safety-class isolated` returns only envs whose `dec:safetyClass` is `isolated`.

4. **Filter by type.** `dec verify env list --type remote-http` returns only envs with that `dec:envType`.

5. **Combined filters.** `dec verify env list --safety-class shared-non-destructive --type remote-http` applies both predicates conjunctively.

6. **JSON shape.** `--format json` emits an array of objects with keys `id`, `env_type`, `safety_class`, `endpoint` (omitted if absent), `allowed_ops`, `setup` (omitted if absent), `teardown` (omitted if absent).

7. **MCP parity.** `dec_verify_env_list` with the equivalent JSON input returns a structured response whose `envs` array matches the CLI JSON output element-for-element.

8. **Invalid filter.** `--safety-class yolo` exits 2 with `Error::InvalidArgument { field: "safety_class" }` on the CLI; MCP returns the structured error.

## Fixture

- A tempdir with three pre-authored envs covering multiple safety classes and types.

## Out of scope

- Pagination (slice 3+).
- Cross-stream listing.
