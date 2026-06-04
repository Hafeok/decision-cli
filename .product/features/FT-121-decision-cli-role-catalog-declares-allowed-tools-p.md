---
id: FT-121
title: 'decision-cli: role catalog declares allowed_tools per dec:Role'
phase: 4
status: complete
depends-on:
- FT-030
adrs:
- ADR-070
- ADR-008
- ADR-071
tests:
- TC-266
- TC-267
- TC-268
domains:
- data-model
- security
domains-acknowledged:
  data-model: One new predicate (`dec:roleTool`, literal range). Seed catalog is extended; no migration of existing seeded roles is required because this feature owns the seed of the new predicate.
  security: The role catalog becomes the source of truth for per-role tool surfaces — i.e. the authorisation policy for what each worker dispatch may invoke. SHACL enforces minCount-1 so no role can ship without a declared surface.
---

## Description

[ADR-070](ADR-070) settles where a dispatch's allowed tool surface comes from: the role catalog. This feature implements that decision on the Rust side. After it lands, every `dec:Role` in the orchestration store carries one or more `dec:roleTool` literals naming a tool the role is allowed to invoke, SHACL refuses unscoped roles, and `role_catalog::lookup(...)` returns the tool list on the `Role` struct.

This is pure catalog plumbing. No worker code changes. No payload changes. Just the predicate, the seed, the lookup, and the SHACL constraint that makes new roles obey.

The predicate is `dec:roleTool` (literal range — short snake_case tool names). The seed:

- **Implementer role** gets `["read_file", "write_file", "run_build", "run_lint", "run_tests"]`.
- **Verifier role** gets `["read_file", "run_build", "run_lint", "run_tests"]` — no `write_file`. Verifiers report failures; they do not write code.

The lookup helper returns `Vec<String>` on `Role`. Legacy stores authored before this seed runs return an empty Vec — the worker fail-closes on empty per [ADR-069](ADR-069). The `dec init --from` path re-seeds; one-shot operators rebuild via the same path.

## Functional Specification

### Inputs

No operator-facing CLI inputs. The seeded values are constants in `crates/decision-cli/src/core/role_catalog/seeds.rs`; the predicate IRI is a constant in `role.rs`.

### Outputs

- `core::role_catalog::Role` gains an `allowed_tools: Vec<String>` field.
- `core::role_catalog::ROLE_TOOL_IRI = "https://decision-cli.dev/ns#roleTool"` exported.
- Seeded named graph for the implementer role carries quads of the form `<role_iri> dec:roleTool "read_file" .` (one quad per tool).
- SHACL shape for `dec:Role` requires `sh:minCount 1` on `dec:roleTool`. Seeding a `dec:Role` without any tool quads → SHACL validation fails.

### Behaviour

1. `Role` struct extends with `allowed_tools: Vec<String>`. Field is populated by a new `collect_allowed_tools(store, &role_iri)` helper that mirrors `collect_input_types` (`role.rs:180`).
2. `lookup()` (`role.rs:53`) calls `collect_allowed_tools` and writes the result onto the returned `Role`. Empty Vec is a legal return value (legacy stores, grandfathered per [ADR-042](ADR-042)).
3. `implementer_role_quads(...)` and `verifier_role_quads(...)` (`seeds.rs:91, 63`) each append `dec:roleTool` quads for their respective tool lists. Seeding is idempotent — repeated `dec init --from` calls produce the same quad set.
4. SHACL shape for `dec:Role` extends with `sh:property [ sh:path dec:roleTool ; sh:minCount 1 ]`. The shape file lives at `crates/decision-cli/src/core/role_catalog/seeds/roles.shacl.ttl` (or wherever the existing role shape lives — implementation discovers).
5. The dispatch payload is NOT modified by this feature. FT-122 threads the resolved Vec from `lookup()` into `DispatchPayloadJson`.

### Acceptance criteria

- Fresh `dec init` against `./streams/decision-cli-development.ttl` seeds both roles with their tool lists.
- `role_catalog::lookup(&store, "implementer")` returns `Role { allowed_tools: vec!["read_file", "write_file", "run_build", "run_lint", "run_tests"], .. }`.
- `role_catalog::lookup(&store, "verifier")` returns the four-tool subset (no `write_file`).
- A SHACL validation pass against a store with a `dec:Role` instance that has no `dec:roleTool` quads fails with a constraint-violation report.
- A SHACL validation pass against a legacy store (pre-seed) does not panic; `lookup()` returns `Role { allowed_tools: vec![], .. }`. Validation may report the shape violation (advisory); the lookup path remains operational.
- All existing role catalog tests continue to pass — the field addition is additive.

## Out of scope

- Tool-call enforcement at the worker. Owned by FT-123.
- Wire-format threading through `DispatchPayloadJson`. Owned by FT-122.
- Sub-resource scoping per tool (`write_file: ["src/**"]`). Out of scope per [ADR-070](ADR-070)'s "alternatives considered".
- Hot-swapping the tool list of an existing seeded role at runtime. Re-seeding via `dec init --from` is the migration path.

## Exit Criteria (Test Coverage)

Per [ADR-013](ADR-013), every behaviour above is asserted by a TC linked to this feature. See `tests:` list in the front-matter once the TCs are authored under this FT.
