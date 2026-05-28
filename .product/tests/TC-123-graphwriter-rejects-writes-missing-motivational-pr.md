---
id: TC-123
title: graphwriter_rejects_writes_missing_motivational_provenance
type: exit-criteria
status: failing
validates:
  features:
  - FT-073
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_123_graphwriter_rejects_writes_missing_motivational_pr
runner-timeout: 120
last-run: 2026-05-28T08:48:27.153613299+00:00
last-run-duration: 0.4s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Description

Exit criterion for FT-073: GraphWriter rejects a write whose artifact lacks motivational provenance and is not a BoundaryArtifact, emits a structured `ProvenanceViolation` Feedback artifact, and the Python defensive validator (pyshacl) reaches the same conformance verdict on the same input.

## Acceptance criteria

- A test attempts to write a `:Feature` instance with mechanical provenance but no motivational edge and no BoundaryArtifact class membership. GraphWriter returns `WriteError::ProvenanceRejected`. The transaction is not committed (a subsequent SELECT confirms the artifact is absent).
- The returned `ProvenanceViolation` payload includes the artifact IRI, the declared type, and the slice-1 motivational predicate set for `:Feature` (from FT-070).
- A Feedback artifact of class `provenance-violation` is emitted and routed to the producing session via the FT-029 routing table.
- The same artifact handed to the Python SDK's pyshacl defensive validator (`workers/_shared/shacl`) produces an equivalent violation report.
- A second test (positive case) writes a conformant `:Feature` and asserts validation passes and the transaction commits within the < 50ms p99 latency budget.

## Runner

`cargo-test` against `crates/decision-cli/tests/ft_073_graphwriter_shacl.rs::rejects_missing_motivational_provenance`. The dual-validator agreement check runs as a Python pytest alongside.