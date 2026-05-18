---
id: FT-007
title: 'decision-cli: Bundled ValueAction and ValueStream template library'
phase: 1
status: planned
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
tests: []
domains: []
domains-acknowledged: {}
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
