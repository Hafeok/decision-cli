---
id: TC-356
title: add-artifact-type coherence audit fails when SHACL shape omits a struct field
type: scenario
status: unimplemented
validates:
  features:
  - FT-141
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-356-cluster-audit-artifact-type-negative.sh
runner-timeout: 60
observes:
- exit-code
- stderr
---

## Context

Scenario TC for [FT-141](FT-141) (TaskType `add-artifact-type`). Negative case for the coherence audit — exercises the **shacl-field-coverage** check (audit check 1 in FT-141 §Outputs). When the SHACL shape drops a `sh:property` block for a field that exists on the Rust struct, the audit MUST exit non-zero and identify the drift.

## Setup

A synthetic six-file fixture committed under `tests/fixtures/cluster-audit-add-artifact-type/negative-shacl-field-coverage/`. The fixture is identical to TC-355's positive case EXCEPT:

- `foo.shacl.ttl` (shacl_shape) — declares `dec:FooShape sh:targetClass dec:Foo` with TWO `sh:property` blocks (`sh:path dec:name`, `sh:path dec:domain`). The block for `sh:path dec:payload` is deliberately OMITTED.

Every other file (`foo.rs`, `foo_vocab.rs`, `parser.rs`, `emitter.rs`, `tests.rs`) is unchanged from TC-355 — `payload` still appears on the Rust struct, in the IRI consts, in the parser, in the emitter, and in the tests. The drift is isolated to the SHACL shape.

## Steps

1. `bash scripts/checks/tc-356-cluster-audit-artifact-type-negative.sh` invokes the audit script with the six fixture file paths.

## Expected outcome

- Audit script exits 1 (audit failure).
- Stderr contains the failing check identifier: `shacl-field-coverage`.
- Stderr names the offending field: `payload`.
- The remaining 5 checks (iri-const-reachability, parser-field-coverage, emitter-field-coverage, round-trip-tests-both-cases, no-python-files) are not asserted on by this TC — the audit MAY short-circuit on the first failure, OR MAY report all failures; both behaviours are acceptable as long as `shacl-field-coverage` is named.

## Pass / fail

- Pass: shell wrapper script asserts non-zero exit + presence of `shacl-field-coverage` and `payload` in the audit's stderr.
- Fail: audit exits 0 (false negative — drift not caught), OR stderr omits the check identifier (drift caught but not named), OR stderr omits the offending field (caught and named but operator can't trace it).

## Why this scenario

This is the audit's most witnessed drift mode per ADR-080's reference features. Of FT-026/035/054/071/086, all five have shown at least one revision where the struct gained a field, the SHACL shape lagged, and the only signal was a runtime SHACL validator failure on real data. Without this check the broad worker's shared-context advantage (everything-sees-everything) returns as cluster's main weakness; with this check, the cluster has structural teeth that the monolith never made explicit. Per ADR-080 §Consequences, "the coherence audit replaces the broad worker's shared context — explicit, structural, testable."
