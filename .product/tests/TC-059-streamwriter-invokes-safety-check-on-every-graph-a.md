---
id: TC-059
title: StreamWriter invokes safety check on every graph and step commit
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 2
---

## Description

[FT-037](FT-037) is the only safety chokepoint per [ADR-028](ADR-028). This TC asserts the chokepoint property — every commit involving a `VerificationGraph` or a `VerificationStep` invokes the safety check before SHACL, and no code path bypasses it.

## Acceptance Criteria

1. **Graph commit invokes graph-level check.** Committing a `VerificationGraph` quad set through `StreamWriter` triggers `check_graph_against_env` against the referenced env. A violating commit is aborted before SHACL runs; the on-disk file is not written.

2. **Step append invokes per-step check.** Committing a single new `VerificationStep` appended to an existing graph's `dec:steps` list triggers `check_step_against_env` against the parent graph's env. Violation aborts the append.

3. **Empty graph trivially passes.** A new graph with `dec:steps ()` passes the safety check unconditionally (no steps to check). The check function returns `Ok(())`.

4. **No safety bypass exists.** A grep-based structural test asserts that `core::ontology::verification_graph::*` types are never constructed and inserted into the store outside the `StreamWriter::commit` path (no rogue `store.insert` calls in feature code).

5. **SHACL and safety errors are distinct.** A graph that violates both SHACL and safety surfaces as `Error::SchemaViolation` first (SHACL runs *after* safety per [FT-037](FT-037) §Behaviour, but both must be reachable); the test confirms both error variants exist and are independently raised.

## Fixture

- Unit tests in `core::stream_writer::safety_integration_tests` using an in-memory store.
- A grep-based structural test under `crates/decision-cli/tests/structural/no_safety_bypass.rs`.

## Out of scope

- Per-violation diagnostic content (TC-058).
- Runtime safety during execution (slice 3).
