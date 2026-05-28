---
id: TC-121
title: boundary_artifact_class_satisfies_motivational_or_branch
type: exit-criteria
status: passing
validates:
  features:
  - FT-071
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_071_boundary_artifact
runner-timeout: 120
last-run: 2026-05-28T08:48:26.392027928+00:00
last-run-duration: 0.3s
---

## Description

Exit criterion for FT-071: declaring an artifact as `:BoundaryArtifact` (or one of its four subclasses) satisfies the type's motivational `sh:or` requirement without needing any motivational predicate, provided `:external_origin` is present.

## Acceptance criteria

- A fixture `:Feature` artifact carrying only a mechanical block + `rdf:type dec:Feature, dec:InitialRequest` + `dec:external_origin "chat-transcript:..."` validates against `:FeatureShape` with no motivational predicate edge.
- The same artifact without `:external_origin` fails validation with the violation report naming the `:external_origin` property path.
- A `:MigrationBackfill` instance lacking `:isMigrationBackfill true` fails the `:MigrationBackfillShape` extension.
- The four subclasses (`SensingActionOutput`, `InitialRequest`, `BootstrapArtifact`, `MigrationBackfill`) are loaded as `rdfs:subClassOf dec:BoundaryArtifact`, verified by SPARQL query over the bootstrap graph.

## Runner

`cargo-test` against `crates/decision-cli/tests/ft_071_boundary_artifact.rs::class_satisfies_motivational_or`.