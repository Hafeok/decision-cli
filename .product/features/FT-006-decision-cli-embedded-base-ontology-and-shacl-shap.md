---
id: FT-006
title: 'decision-cli: Embedded base ontology and SHACL shapes'
phase: 1
status: planned
depends-on: []
adrs:
- ADR-007
- ADR-008
- ADR-001
- ADR-002
- ADR-004
- ADR-005
- ADR-012
tests: []
domains: []
domains-acknowledged: {}
---

## Description

decision-cli ships a base ontology embedded as a static asset per **ADR-007 (Embedded base ontology and bundled templates)**, declaring `dec:ValueStream`, `dec:ValueAction`, `dec:Goal`, `dec:Session`, `dec:Dispatch`, `dec:Event` with SHACL shapes constraining required fields. Every `dec init` records the ontology version it used.

This ontology is what the **ADR-006** validation pipeline (parse / SHACL / resolve / cross-validate / persist) runs against.

See `decision-cli-slice-1-bounds.md` §3.1, §5.3.

## Functional Specification

### Inputs

- The ontology Turtle/JSON-LD bytes compiled into the binary as embedded assets.
- A runtime request for the ontology graph (from FT-008, FT-009, FT-010).

### Outputs

- An `OntologyHandle` exposing the ontology triples and SHACL shapes graph for validators.
- A version identifier and content hash.

### State

- Static-embedded bytes (e.g., `include_bytes!`).
- An on-demand parsed in-memory graph cached per process.

### Behaviour

1. At binary start, embedded bytes are available but not parsed until first request.
2. First request parses into an Oxigraph in-memory graph, validates structural well-formedness, computes the content hash.
3. The handle exposes `shapes_graph()`, `version()`, `hash()`, and class/property lookup helpers.

### Invariants

- Runtime ontology version matches the version compiled into the binary (ADR-007).
- Declares at minimum the classes named in §5.3 with SHACL shapes for their required fields.
- `dec:ValueStream` shape requires `dec:name`, `dec:title`, `dec:terminalValueAction`, `dec:authorizedGoals`. `dec:ValueAction` shape requires name, description, exit criteria, expected output types, compatible goals.

### Error handling

- Parse failure on the embedded asset is treated as a build-time bug — fatal `OntologyError::CompiledAssetMalformed`; should be impossible in a correctly-built binary.

### Boundaries

- This feature does NOT validate user-supplied definition documents — that is FT-008.
- This feature does NOT persist ontology triples to the orchestration store — FT-009.
- This feature does NOT define ValueAction/ValueStream instance documents — FT-007.

## Out of scope

- Loading ontology from external files at runtime (ADR-007 enforces static embedding for slice 1).
- Ontology evolution / migration tooling.
- Custom user extensions to the base ontology.
