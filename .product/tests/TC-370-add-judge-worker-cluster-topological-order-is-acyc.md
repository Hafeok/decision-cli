---
id: TC-370
title: add-judge-worker cluster topological order is acyclic and deterministic
type: exit-criteria
status: passing
validates:
  features:
  - FT-139
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --lib core::task_type::tests::add_judge_worker_topo_order
runner-timeout: 60
observes:
- exit-code
last-run: 2026-06-04T14:35:45.762132444+00:00
last-run-duration: 0.2s
---

## Acceptance criteria

Verifies that [FT-139](FT-139)'s `add-judge-worker` TaskType declares a cluster whose `derived_from` edges form an acyclic, topologically-sortable graph with deterministic order. Locks in [ADR-080](ADR-080)'s requirement that cluster ordering is data, not emergent.

### Conditions

Unit test in `crates/decision-cli/src/core/task_type/tests.rs`.

- Construct or load the `add-judge-worker` `TaskTypeDecl` from the static registry.
- Call `Cluster::topo_order(&task_type.cells)`.
- Assert: returns `Ok(order)` (no cycle).
- Assert: every cell name appears exactly once in `order`.
- Assert: for every cell `c` and every `dep` in `c.derived_from`, `dep` appears in `order` BEFORE `c`.
- Assert: re-running `topo_order` over the same cells produces a byte-identical `Vec<String>` (deterministic — the implementation must not depend on HashMap iteration order).

### Exit codes

- `0` — order is acyclic, complete, dependency-respecting, deterministic.
- `1` — cycle detected, missing cell, dependency-order violation, or non-deterministic output.

### Surface

`exit-code` — pure unit test, no I/O.