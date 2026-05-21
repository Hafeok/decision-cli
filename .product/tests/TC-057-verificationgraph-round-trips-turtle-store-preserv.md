---
id: TC-057
title: VerificationGraph round-trips Turtle ↔ store preserving step order
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
---

## Description

[FT-036](FT-036) stores graphs as raw Turtle at `.dec/verify/graph/VG-NNN.ttl` and projects them into the orchestration store. Step order is the `dec:steps` rdf:List order. This TC asserts the round-trip preserves the structure exactly — same Turtle in, same Turtle back out, same projection in the store.

## Acceptance Criteria

1. **Header round-trip.** A graph with `dec:verifies <ft:FT-001>`, `dec:environment <env:ephemeral-cli>`, and `dec:steps ()` (empty) parses, projects, and reserialises to canonical Turtle whose semantic content equals the input (modulo blank-node naming and prefix declarations).

2. **Ordered step list.** A graph with three steps in order (`shell-command`, `sparql-assertion`, `file-assertion`) round-trips with the same step kinds in the same order — the in-memory `Vec<VerificationStep>` mirrors the rdf:List position-for-position.

3. **Step IRI stability.** Step IRIs are derived deterministically from `(graph_id, index)`. Reloading the same Turtle produces the same step IRIs every time.

4. **Polymorphic dec:verifies.** A graph with `dec:verifies <tc:TC-013>` round-trips identically to one with `dec:verifies <ft:FT-013>`; SHACL accepts both.

5. **${name} preserved verbatim.** A step with `dec:command "dec verify ${earlier_capture}"` reserialises with the literal `${earlier_capture}` intact — no resolution, no warning.

6. **Reload from disk yields equal projection.** Parse a Turtle file → project into store → dump store → parse dump → equal triples (modulo blank-node renaming) as the original parse.

## Fixture

- Canonical Turtle fixtures under `crates/decision-cli/tests/fixtures/verify_graph/`.
- Property-based test: random graph structures (size ≤ 10 steps) round-trip.

## Out of scope

- SHACL invariants per kind (TC-056).
- Safety enforcement (TC-058, TC-059).
- Authoring CLI surface (FT-041..FT-044 TCs).
