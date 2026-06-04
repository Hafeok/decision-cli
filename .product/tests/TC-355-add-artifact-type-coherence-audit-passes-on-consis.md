---
id: TC-355
title: add-artifact-type coherence audit passes on consistent positive fixture
type: scenario
status: unimplemented
validates:
  features:
  - FT-141
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-355-cluster-audit-artifact-type-positive.sh
runner-timeout: 60
observes:
- exit-code
- stderr
---

## Context

Scenario TC for [FT-141](FT-141) (TaskType `add-artifact-type`). Positive case for the coherence audit script `scripts/checks/cluster-audit-add-artifact-type.py`: when all six cell outputs are mutually consistent (field set ↔ SHACL `sh:path` set ↔ IRI constants ↔ parser LHS ↔ emitter RHS ↔ round-trip tests cover both cases), the audit exits 0.

## Setup

A synthetic six-file fixture representing a hypothetical `Foo` artifact type. The fixture is committed under `tests/fixtures/cluster-audit-add-artifact-type/positive/`:

- `foo.rs` (rust_struct):
  ```rust
  #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
  pub struct Foo {
      pub name: String,
      pub domain: String,
      pub payload: String,
  }
  ```
- `foo.shacl.ttl` (shacl_shape) — declares `dec:FooShape sh:targetClass dec:Foo` with three `sh:property` blocks, one per field (`sh:path dec:name`, `sh:path dec:domain`, `sh:path dec:payload`).
- `foo_vocab.rs` (iri_module_consts) — three `NamedNode` constants: `FOO_NAME`, `FOO_DOMAIN`, `FOO_PAYLOAD`.
- `parser.rs` — function with three LHS assignments: `let name = ...`, `let domain = ...`, `let payload = ...`, each referencing the corresponding IRI const.
- `emitter.rs` — function producing quads with three RHS field accesses: `foo.name`, `foo.domain`, `foo.payload`, each paired with the corresponding IRI const.
- `tests.rs` — one positive round-trip test (constructs `Foo`, emits, parses, asserts equality) AND one negative SHACL test (constructs malformed `Foo` instance missing `payload`, runs SHACL validator, asserts rejection).

## Steps

1. `bash scripts/checks/tc-355-cluster-audit-artifact-type-positive.sh` invokes the audit script with the six fixture file paths as CLI args.

## Expected outcome

- Audit script exits 0.
- Stderr is empty (no audit-failure messages).
- All six checks pass:
  1. shacl-field-coverage: `{name, domain, payload}` ⊆ SHACL `sh:path` set.
  2. iri-const-reachability: every IRI const referenced by parser or emitter.
  3. parser-field-coverage: every field assigned in parser.
  4. emitter-field-coverage: every field written in emitter.
  5. round-trip-tests-both-cases: positive + negative both present.
  6. no-python-files: zero `.py` files in inputs.

## Pass / fail

- Pass: shell wrapper script exits 0.
- Fail: script exits non-zero or stderr contains audit-failure markers.

## Why this scenario

This is the audit's load-bearing positive case: it proves the audit doesn't false-positive on a fully consistent cluster. Without this, an audit that always-fails would technically catch every divergence but would also block every legitimate cluster — useless. Per ADR-080 §Decision §3, the audit is the load-bearing property of the whole pattern; the positive case is the proof it isn't pathologically strict.
