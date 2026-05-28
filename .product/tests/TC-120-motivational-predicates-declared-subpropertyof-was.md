---
id: TC-120
title: motivational_predicates_declared_subpropertyof_wasderivedfrom
type: exit-criteria
status: passing
validates:
  features:
  - FT-070
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_070_motivational_vocabulary
runner-timeout: 120
last-run: 2026-05-28T08:48:26.027202779+00:00
last-run-duration: 0.4s
---

## Description

Exit criterion for FT-070: every predicate declared in `motivational-predicates.ttl` carries `rdfs:subPropertyOf prov:wasDerivedFrom`, and a SPARQL query over `prov:wasDerivedFrom*` walks instances of every per-type motivational predicate uniformly.

## Acceptance criteria

- A test parses the shipped `motivational-predicates.ttl` and asserts every `rdf:Property` declaration in the file has `rdfs:subPropertyOf prov:wasDerivedFrom`.
- The set of predicates in the file matches the slice-1 vocabulary table in FT-070's body (single source of truth fitness check).
- A fixture graph asserting `:f1 :addresses :feedback1`, `:f1 :decomposesFrom :brief1`, `:adr1 :decidesFor :f1` produces three results from a `SELECT ?ancestor WHERE { :f1 prov:wasDerivedFrom* ?ancestor }` query (without the test author enumerating any motivational predicate name).
- The build-time range-agreement fitness check passes for the slice-1 type shapes: for every per-type `sh:property [ sh:path :foo ; sh:class :Bar ]` in FT-072 files, the declaration of `:foo` includes `:Bar` in its range.

## Runner

`cargo-test` against `crates/decision-cli/tests/ft_070_motivational_vocabulary.rs::predicates_are_subproperties_and_walkable`.