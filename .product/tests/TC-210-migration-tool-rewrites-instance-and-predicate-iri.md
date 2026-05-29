---
id: TC-210
title: Migration tool rewrites instance and predicate IRIs in store atomically
type: scenario
status: unimplemented
validates:
  features:
  - FT-112
  adrs: []
observes:
- graph
phase: 4
runner: cargo-test
runner-args: tc_210_migration_rewrites_instance_and_predicate_iris
runner-timeout: 60
---

## Description

The store migration is the only step that mutates persisted
data; correctness here means every old IRI gets a corresponding
new IRI and no other quad changes. PAT-001's "rule is a pure
function" discipline applies: the migration is a `(store_in,
store_out)` transformation testable without spinning up the
real CLI process.

## Acceptance Criteria

Cargo test that:

1. Builds a temp orchestration store with:
   - 1 × VerificationEnvironment instance at
     `https://decision-cli.dev/ns/env/ENV-001`.
   - 2 × VerificationGraph quads with
     `dec:environment <ENV-001>` predicate.
   - 1 × VerificationGraphResult with
     `dec:ranInEnvironment <ENV-001>`.
   - 1 × unrelated quad mentioning `prov:wasGeneratedBy`
     (control — should not be touched).
2. Calls the migration entry point
   (`core::migrate::env_to_bench(store)` or whatever the
   public API is).
3. Asserts the store now contains:
   - 1 × VerificationBench instance at
     `https://decision-cli.dev/ns/bench/BNCH-001`.
   - 2 × VerificationGraph quads with `dec:bench <BNCH-001>`.
   - 1 × VerificationGraphResult with `dec:ranOnBench <BNCH-001>`.
   - The control quad is unchanged.
4. Asserts NO quad in the store still mentions any string with
   `/ns/env/` or `#envType` or `#ranInEnvironment` or
   `#VerificationEnvironment`.

Cardinality preservation: pre-migration quad count == post-
migration quad count (verifies no quad is dropped or duplicated).
