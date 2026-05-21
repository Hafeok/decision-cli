---
id: TC-054
title: VerificationEnvironment SHACL shape rejects malformed env
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
---

## Description

[FT-035](FT-035)'s `dec:VerificationEnvironmentShape` enforces structural invariants per [ADR-028](ADR-028): non-empty `envType`, safety class in the controlled vocabulary, non-empty `allowedOps`, and conditional `endpoint` for remote types. This TC exercises each invariant in isolation.

## Acceptance Criteria

1. **Well-formed env passes.** A canonical `ephemeral-cli` env Turtle (envType `ephemeral-tempdir`, safetyClass `isolated`, allowedOps `(shell filesystem sparql-local)`) commits through `StreamWriter` without error.

2. **Missing envType.** Committing an env without `dec:envType` fails with `Error::SchemaViolation { artifact: EnvId, detail }`; the detail string names `envType`.

3. **Unknown safety class.** Committing an env with `dec:safetyClass "yolo"` fails; the detail string names `safetyClass` and lists the three accepted values.

4. **Empty allowedOps.** Committing an env with `dec:allowedOps ()` (empty list) fails; the detail string names `allowedOps`.

5. **Remote env without endpoint.** Committing an env with `dec:envType "remote-http"` and no `dec:endpoint` fails; the detail names `endpoint`. A `remote-http` env *with* an `endpoint` commits successfully.

6. **Local env with endpoint is rejected.** An env with `dec:envType "ephemeral-tempdir"` and a `dec:endpoint` value fails; SHACL conditional rejects spurious endpoints on local types.

## Fixture

- A `StreamWriter` test harness writing into an in-memory store with the ontology bundle loaded.
- One Turtle fragment per case, isolated.

## Out of scope

- The seed `ephemeral-cli` env content (TC-055).
- Round-trip parse/serialise (covered indirectly via TC-057).
