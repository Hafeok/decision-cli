---
id: TC-368
title: extend-role-catalog-seed coherence audit fails when seed_quad_function references undeclared IRI constant
type: scenario
status: unimplemented
validates:
  features:
  - FT-144
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-368-cluster-audit-role-catalog-seed-missing-iri.sh
runner-timeout: 60
observes:
- exit-code
- stderr
---

## Context

Negative coherence-audit TC for [FT-144](FT-144) — asserts the `extend-role-catalog-seed` audit catches the case where `seed_quad_function` references an IRI constant that was not declared by `iri_constants`. This is the audit's check #1 firing: every IRI constant must be referenced by the seed function, AND inversely (interpreted here) every IRI reference in the seed function must resolve to a declared constant — otherwise the code does not compile in real life, and a dropped-constant or typo-in-reference produces a silent half-cluster.

Per [ADR-080](ADR-080)'s safety property: *"The coherence audit is the load-bearing audit of the whole pattern."* This TC proves audit check #1 has teeth for missing-IRI failures.

## Setup

- A fixture directory under `tests/fixtures/cluster-audit-extend-role-catalog-seed/missing-iri/` identical to the positive fixture (TC-367) EXCEPT:
  - `seed_quad_function.rs` references `BAZ_IRI` (a constant name that does NOT appear in `iri_constants.rs`). Concretely: change one occurrence of `FOO_IRI` to `BAZ_IRI` inside the seed quad function body.
- All other cells (init_pipeline_wiring, shacl_shape_extension, role_struct_field_extension, round_trip_tests) are internally consistent with the positive fixture so the audit's only failure signal is the IRI mismatch.
- The wrapper script `scripts/checks/tc-368-cluster-audit-role-catalog-seed-missing-iri.sh` invokes the audit with all six paths and `--params '{"requires_shacl": true, "surfaces_on_role_struct": true}'`, capturing exit code + stderr.

## Steps

1. Run `scripts/checks/cluster-audit-extend-role-catalog-seed.py` against the missing-iri fixture via the wrapper.
2. Capture exit code and stderr.

## Expected outcome

- Exit code 1 (audit failure, not unrunnable).
- Stderr contains a FAIL line for check #1 — substring match: `FAIL iri_constants_referenced_by_seed_quads` (or the canonical check-1 identifier as named by the audit script).
- The FAIL detail names the offending undeclared IRI reference (substring `BAZ_IRI`) so the operator can locate it without grepping.
- Other checks (2..6) may PASS or FAIL depending on cascading effects of the IRI rename; check #1 firing with the canonical identifier is the binding assertion.

## Pass / fail

- Pass: bash runner exits 0 because the wrapper asserts the audit exited 1 AND stderr matched both the check-1 identifier AND the `BAZ_IRI` substring.
- Fail: the audit script unexpectedly exits 0 (audit has no teeth for IRI mismatches) OR exits 2 (unrunnable — fixture broken) OR exits 1 without naming check #1.

## Why this TC

This is the missing-constant case the broad code-writer would have caught implicitly via compilation (an unresolved `BAZ_IRI` does not compile). Decomposing into cells means the cells emit independently; until the cluster is assembled and built, the missing-reference is silent. The audit must restore that signal before the worktree merges — otherwise the cluster's decomposition is strictly worse than the monolith per ADR-080's load-bearing test.