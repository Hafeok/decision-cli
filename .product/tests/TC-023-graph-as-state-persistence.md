---
id: TC-023
title: orchestration_store_persisted_as_graph_dump
type: invariant
status: passing
validates:
  features: []
  adrs:
  - ADR-002
phase: 1
runner: bash
runner-args: scripts/checks/graph-as-state.sh
runner-timeout: 60
last-run: 2026-05-20T11:41:36.841111001+00:00
failure-message: |
  ERROR: expected crates/decision-cli/src/init/persist.rs (ADR-002 anchor file)
last-run-duration: 0.0s
---

## Purpose

Mechanical enforcement of **ADR-002 graph-as-state**. Asserts that
`crates/decision-cli/src/init/persist.rs` still serialises the
orchestration store via `RdfFormat::NQuads` to `orchestration.nq`.

The decision behind ADR-002 is that the persisted graph **is** the
state; we deliberately do not maintain a parallel event log that needs
replay to derive current state. A regression here would mean we have
quietly introduced an event-sourced reducer pattern.

## Given

- A working copy of decision-cli with `crates/decision-cli/src/init/persist.rs`
  present.
- `bash` and `grep` available on `PATH`.

## When

```bash
scripts/checks/graph-as-state.sh
```

## Then

1. Exit 0 if the persistence module still writes the orchestration
   store as an N-Quads dump.
2. Exit 1 if the `RdfFormat::NQuads` serialisation or the
   `orchestration.nq` path has been removed (graph-as-state regression).

## Formal Specification

⟦Γ:Invariants⟧{
  references(crates/decision-cli/src/init/persist.rs, RdfFormat::NQuads)
  references(crates/decision-cli/src/init/persist.rs, "orchestration.nq")
  ¬ ∃ EventLogReducer ∈ crates/decision-cli/src/**.rs
}
