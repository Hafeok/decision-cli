---
id: FT-007
title: 'decision-cli: Bundled ValueAction and ValueStream template library'
phase: 1
status: complete
depends-on:
- FT-006
adrs:
- ADR-007
- ADR-008
- ADR-001
- ADR-002
- ADR-004
- ADR-005
- ADR-012
tests:
- TC-001
- TC-004
- TC-005
domains: []
domains-acknowledged:
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-007 neither emits nor routes feedback.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-007 produces no feedback artifacts.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-007 neither emits nor consumes verdicts.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-007's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-007 does not introduce or modify a role catalog entry.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-007 produces no action/interpretation pair.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-007 has no feedback to gate.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-007 is out of scope for the pairing.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-007's code is reorganised under that migration, not by this feature.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-007 does not author or modify a fitness-function artifact.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-007 produces no feedback artifacts.
---

## Description

decision-cli ships a bundled library of canonical `ValueAction` definitions and `ValueStream` templates per **ADR-007**. Each is addressed by a stable URI; lookup is offline (slice 1 supports only the bundled set per **ADR-006**).

Slice 1 includes at minimum `va:shipped-feature` (the terminal value action for `decision-cli-development`) and the `engineering-development` template.

See `decision-cli-slice-1-bounds.md` §3.1, §3.2, §6.1.

## Functional Specification

### Inputs

- A request for a bundled ValueAction by URI (e.g., `https://decision-cli.dev/ns/value-actions/shipped-feature`).
- A request for a bundled ValueStream template by name (e.g., `engineering-development`).

### Outputs

- A parsed, SHACL-valid `ValueAction` artifact for the requested URI, or a "not bundled" error.
- A parsed `ValueStream` template ready for FT-008 to instantiate.

### State

- Bundled Turtle/JSON-LD assets compiled into the binary alongside FT-006's ontology.
- An in-memory index URI → asset bytes built once at first use.

### Behaviour

1. On lookup, fetch the bundled bytes by URI/name.
2. Parse to an in-memory graph; validate against the SHACL shapes from FT-006.
3. Return the parsed artifact.

### Invariants

- Every bundled ValueAction SHACL-validates against the ontology shipped in the same binary build.
- Every bundled ValueStream template references a ValueAction URI within the bundled set.
- `va:shipped-feature` is always present in slice 1.

### Error handling

- Unknown URI → `BundleError::Unknown(uri)`. FT-008 surfaces this as the "unbundled URI" error.
- A bundled definition failing its own SHACL is a build-time bug; panics on first parse.

### Boundaries

- This feature does NOT fetch from URLs — ADR-006 defers that.
- This feature does NOT define a registry server.
- This feature does NOT validate user-supplied stream documents (FT-008).

## Out of scope

- Network resolution / registry fetch.
- ValueStream composition / extension.
- Versioning of bundled definitions beyond the binary version.
