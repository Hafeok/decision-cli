---
id: TC-267
title: SHACL refuses a dec:Role with no dec:roleTool quads
type: scenario
status: passing
validates:
  features:
  - FT-121
  adrs:
  - ADR-070
phase: 4
observes:
- graph
runner: cargo-test
runner-args: tc_267_shacl_refuses_role_without_role_tool
runner-timeout: 30
last-run: 2026-06-04T09:05:26.005362752+00:00
last-run-duration: 0.5s
---

## Description

[ADR-070](ADR-070) requires every `dec:Role` to declare at least one `dec:roleTool`. The SHACL shape extended by FT-121 carries `sh:property [ sh:path dec:roleTool ; sh:minCount 1 ]`. This TC asserts the constraint is wired up: an artificially-constructed `dec:Role` quad-set that omits `dec:roleTool` triggers a SHACL violation when validated.

This is the gate that keeps future role authors honest. Without it, a new role could ship without a tool surface and the worker would silently fall through to fail-closed at runtime instead of failing the seed.

## Acceptance Criteria

Construct an in-memory `Store` containing only:

- A `dec:Role` instance with `dec:roleId`, `dec:roleOutputType`, `dec:roleInputType`, `dec:roleModelBinding` — i.e. everything *except* `dec:roleTool` quads.

Run SHACL validation against the role shape (the canonical shape file shipped by FT-121). The result MUST:

- Report at least one constraint violation.
- The violation's `sh:resultPath` MUST be `dec:roleTool`.
- The violation's `sh:sourceConstraintComponent` MUST be `sh:MinCountConstraintComponent`.
- The violation's `sh:focusNode` MUST be the offending role IRI.

Lives at `crates/decision-cli/src/core/role_catalog/tests.rs::tc_267_shacl_refuses_role_without_role_tool`. Uses the existing oxigraph SHACL validation helper used by the other role-shape tests in that module.