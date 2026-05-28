---
id: TC-166
title: OntologyDescription enforces single-active invariant; parallel non-superseded write is rejected by SHACL
type: scenario
status: failing
validates:
  features:
  - FT-101
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_166_ontologydescription_enforces_single_active_invaria
runner-timeout: 120
last-run: 2026-05-28T08:49:01.807930442+00:00
last-run-duration: 0.9s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Claim

`dec catalog ontology new` enforces the single-active-ontology invariant: at most one `dec:OntologyDescription` per stream is non-superseded at any time. A `new` that would create a parallel active description is rejected by the StreamWriter's SHACL pass before persistence.

## Scenarios

### Setup

- Fresh `.dec/` initialised via `dec init`.
- Empty `.dec/catalog/ontology/`.

### Scenario A — first ontology description succeeds

Invoke `dec catalog ontology new OD-001 --namespace "https://decision-cli.dev/ns#" --version 0.3.0 --from-file fixtures/od-001-body.json`. Assertions:

- Exit code: 0.
- File `.dec/catalog/ontology/OD-001.ttl` exists.
- `dec catalog ontology list --active` returns `[OD-001]`.

### Scenario B — parallel active write is SHACL-rejected

Invoke `dec catalog ontology new OD-002 --namespace "https://decision-cli.dev/ns#" --version 0.3.1 --from-file fixtures/od-002-body.json`. Assertions:

- Exit code: 1.
- Stderr contains `SchemaViolation` and references the SHACL constraint `sh:sparql` enforcing the single-active rule (the exact constraint name is implementation-defined; the test asserts on the substring `single-active` or `OntologyDescription` plus the SHACL violation marker).
- File `.dec/catalog/ontology/OD-002.ttl` is **not** created.
- `dec catalog ontology list --active` still returns `[OD-001]` (unchanged).

### Scenario C — supersession unblocks a fresh active author

Invoke `dec catalog ontology supersede OD-001 --by OD-002 --new-file fixtures/od-002-body.json --new-version 0.3.1`. Assertions:

- Exit code: 0.
- `OD-001` carries `dec:supersededBy <OD-002>`.
- `OD-002` carries `dec:supersedes <OD-001>` and is the unique active description.
- `dec catalog ontology list --active` returns `[OD-002]`.
- `dec catalog ontology list --include-superseded` returns `[OD-001, OD-002]`.

### Scenario D — referenced-but-undeclared predicate is write-time rejected

Write a body for `OD-003` containing a predicate (e.g. `dec:fakePredicate`) that is not present in the SHACL shapes shipped by [FT-006](FT-006). After superseding OD-002, invoke `dec catalog ontology new OD-003 --from-file fixtures/od-003-fake-predicate-body.json`. Assertions:

- Exit code: 1.
- Stderr names `dec:fakePredicate` as the missing predicate.
- The error message points the operator at the shape file they need to extend before re-authoring.

## Runner

`bash tests/scripts/tc-166-ontology-single-active.sh`. Temp `.dec/`, fixture JSON bodies for OD-001/002/003 ship alongside the script.

## Non-goals

- The `ontology_vocabulary` field of the worker bundle (FT-102 TC).
- Multi-stream sharing of ontology descriptions (out of slice).
- Live SHACL shape regeneration when an OD is superseded (a separate concern — the shapes are pinned at the embedded-ontology bundle level, not derived from the OD).