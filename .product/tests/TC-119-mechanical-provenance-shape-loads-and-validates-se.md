---
id: TC-119
title: mechanical_provenance_shape_loads_and_validates_session_record
type: exit-criteria
status: unimplemented
validates:
  features:
  - FT-069
  adrs: []
phase: 1
---

## Description

Exit criterion for FT-069: the universal mechanical-provenance SHACL fragment loads cleanly at orchestration-store bootstrap and validates a synthetic artifact whose mechanical block was materialised by GraphWriter from a session record.

## Acceptance criteria

- `dec init` succeeds on a fresh store with FT-069's `mechanical-provenance.ttl` loaded.
- A Rust integration test constructs a fixture artifact, hands a `SessionRecord` to GraphWriter, observes the materialised mechanical block (`prov:wasGeneratedBy`, `prov:wasAttributedTo`, `prov:generatedAtTime`), and asserts SHACL validation against `:MechanicalProvenanceShape` reports `conforms = true`.
- A negative test deliberately strips `prov:wasGeneratedBy` from the same fixture and asserts the validator reports a violation naming that property path.
- The shape file's IRI constants exposed in `core/ontology/` match the IRIs in the TTL byte-for-byte.

## Runner

`cargo-test` against `crates/decision-cli/tests/ft_069_mechanical_provenance.rs::loads_and_validates_session_record`.
