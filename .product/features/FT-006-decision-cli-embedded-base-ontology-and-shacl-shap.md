---
id: FT-006
title: 'decision-cli: Embedded base ontology and SHACL shapes'
phase: 1
status: complete
depends-on: []
adrs:
- ADR-007
- ADR-008
tests:
- TC-001
- TC-003
domains: []
domains-acknowledged:
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-006 produces no action/interpretation pair.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-006 does not author or modify a fitness-function artifact.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-006 does not introduce or modify a role catalog entry.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-006 produces no feedback artifacts.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-006's code is reorganised under that migration, not by this feature.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-006 neither emits nor routes feedback.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-006 produces no feedback artifacts.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-006 neither emits nor consumes verdicts.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-006's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-006 has no feedback to gate.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-006 is out of scope for the pairing.
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
