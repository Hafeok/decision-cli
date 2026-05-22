---
id: FT-053
title: 'decision-cli: dec:fixtureSource env field + --fixture-source CLI flag'
phase: 2
status: complete
depends-on:
- FT-035
- FT-038
- FT-039
- FT-040
adrs:
- ADR-032
- ADR-028
tests:
- TC-097
- TC-098
- TC-099
domains: []
domains-acknowledged: {}
---

## Description

Extend `dec:VerificationEnvironment` ([FT-035](FT-035), [ADR-028](ADR-028)) with the optional `dec:fixtureSource` predicate per [ADR-032](ADR-032). Add `--fixture-source <path>` to `dec verify env new` and the matching MCP argument. Render the value in `dec verify env list` and `dec verify env show`. Validate at authoring time that the path is repo-relative and resolves to an existing directory.

This feature is **schema- and authoring-surface only**. The runner-contract behaviour (copy fixture tree, augment `$PATH`) is out of scope and belongs to whichever future feature ships the step executor (slice 3+).

## Functional Specification

### Inputs

- The `core::ontology::verification_env` module — gains a new optional field on `VerificationEnvironment`.
- The embedded ontology — gains an optional SHACL constraint on `VerificationEnvironmentShape`.
- The `EnvNewRequest` shape in `features::verify_env_new` — gains an optional field.
- The CLI in `cli::verify::env_new` — gains a flag.
- The MCP input schema for `dec_verify_env_new` — gains an optional property.
- The list/show projections in `features::verify_env_list` and `features::verify_env_show` — gain a passthrough field.

### Outputs

- New optional field on `VerificationEnvironment`:
  - `fixture_source: Option<String>` — repo-relative path; `None` when unset.
- New IRI in `core::vocab`:
  - `IRI_DEC_FIXTURE_SOURCE` = `https://decision-cli.dev/ns#fixtureSource`.
- SHACL shape extension on `VerificationEnvironmentShape`:
  - Optional property `dec:fixtureSource`, max-count 1, datatype `xsd:string`, min-length 1.
- CLI surface extension on `dec verify env new`:
  - `--fixture-source <path>` — optional. Passes through into `EnvNewRequest::fixture_source`.
- MCP tool argument extension on `dec_verify_env_new`:
  - Optional property `fixture_source` (string, min-length 1).
- Rendering changes:
  - `dec verify env list` includes `fixture_source` in JSON output when set; text/table format gains an `FIXTURE` column or trailing row per env.
  - `dec verify env show` shows a `Fixture Source: <path>` row in text; `fixture_source` field in JSON.

### State

- Same on-disk Turtle file under `.dec/verify/env/<id>.ttl`. The canonical Turtle gains an optional `dec:fixtureSource "<path>" ;` line, positioned between `dec:safetyClass` and `dec:allowedOps` (alphabetical-by-IRI is the existing canonical-Turtle order; `fixtureSource` slots in there).
- Same store projection — adds one optional quad per env.

### Behaviour

1. CLI / MCP request includes optional `fixture_source`.
2. `EnvNewRequest::fixture_source` flows through the single handler unchanged.
3. `build_env` populates `VerificationEnvironment::fixture_source`.
4. **Path validation** (in `validate::pre_validate`): when `fixture_source` is `Some(p)`:
   - Non-empty after trim.
   - Relative path: rejects any starting with `/`, any whose normalised form contains a `..` segment.
   - `workdir.join(p)` exists and is a directory (not a regular file; not a symlink whose target is not a directory).
   - Failures surface as `Error::InvalidArgument { field: "fixture_source", detail }` — exit 2.
5. `to_quads` emits the optional `dec:fixtureSource` quad when `Some(_)`.
6. `to_canonical_turtle` writes the optional line when `Some(_)`; preserves stable ordering relative to other env fields.
7. `from_turtle` round-trips the value when present.
8. SHACL validates the predicate per the shape extension; absence is fine, multiple values fail the commit with `SchemaViolation`.

### Invariants

- Turtle round-trip is byte-identical for any env with or without `fixture_source`.
- Adding `fixture_source` to an env that didn't have it requires re-creating the env (consistent with the rest of the env schema — there is no `dec verify env edit` in slice 2).
- The path is validated relative to the **authoring workdir**, not the execution workdir. The execution-time presence check is the runner's responsibility (future feature).
- Existing envs without `fixture_source` continue to load, list, show, and project unchanged.

### Error handling

- Absolute path → `InvalidArgument { field: "fixture_source", detail: "fixture_source must be repo-relative" }`.
- Path containing `..` segments → `InvalidArgument { field: "fixture_source", detail: "fixture_source must not contain `..`" }`.
- Path resolves to a non-existent location → `InvalidArgument { field: "fixture_source", detail: "fixture_source <path> does not exist" }`.
- Path resolves to a non-directory → `InvalidArgument { field: "fixture_source", detail: "fixture_source <path> is not a directory" }`.
- Empty / whitespace-only string slipped past validation → `SchemaViolation { detail }` (SHACL min-length 1 catches it).

### Boundaries

- **In scope.** Schema field, IRI, SHACL extension, CLI flag, MCP arg, list/show passthrough, authoring-time path validation.
- **Out of scope.** Runner-side copy + `$PATH` augmentation (future feature). Editing an existing env's `fixture_source` (rest of env schema also lacks edit). Fixture content validation (whatever the fixture contains is the author's responsibility). Multi-fixture composition. A `dec verify fixture` subcommand surface.

## Out of scope

- The graph step executor that consumes `fixture_source` at runtime.
- Editing an existing env to add/remove `fixture_source`.
- Validating that the fixture tree is non-empty, contains specific structure, or has a particular `bin/` layout.
- Multi-fixture overlays or fixture inheritance.
- A `dec verify fixture` subcommand surface.
- Promoting fixtures into graph-native artifacts (rejected per [ADR-032](ADR-032) §Rejected alternatives).
