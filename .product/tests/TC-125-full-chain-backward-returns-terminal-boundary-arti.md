---
id: TC-125
title: full_chain_backward_returns_terminal_boundary_artifacts
type: exit-criteria
status: unimplemented
validates:
  features:
  - FT-075
  adrs: []
phase: 1
---

## Description

Exit criterion for FT-075: the `qt:full-chain-backward-v1` QueryTemplate, executed against a fixture provenance graph, returns the expected terminal ancestors (BoundaryArtifact-class nodes and nodes with no further `prov:wasDerivedFrom` edges).

## Acceptance criteria

- Fixture graph contains a chain: `:focal :addresses :feedback1`, `:feedback1 :observedIn :session1`, `:session1 prov:used :brief1`, `:brief1 rdf:type dec:Brief, dec:InitialRequest` (a boundary-terminating Brief).
- Mechanical edges are also seeded: `:focal prov:wasGeneratedBy :sessionFocal`, `:sessionFocal prov:used :brief1`.
- Helper `store.fetch_query_template("qt:full-chain-backward-v1")` succeeds; SHACL validation of the fetched instance passes against `:QueryTemplateShape`.
- Executing the template with `?focal := :focal` returns at minimum the row `(:brief1, dec:Brief, motivational)` (or `mechanical`, depending on the path taken — both branches must reach the terminal).
- The forward template `qt:full-chain-forward-v1` from the same fixture returns symmetric results when executed on `:brief1`.
- `dec query template list` shows both templates; `dec query template show qt:full-chain-backward-v1` prints the SPARQL spec and `version: "1.0.0"`.

## Runner

`cargo-test` against `crates/decision-cli/tests/ft_075_full_chain_query.rs::backward_returns_terminal_boundary_artifacts`.
