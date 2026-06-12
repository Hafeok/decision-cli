---
id: FT-181
title: 'decision-cli: parallel cell dispatch — cluster topo levels execute concurrently under a bounded worker pool'
phase: 4
status: complete
depends-on:
- FT-177
- FT-178
adrs:
- ADR-080
- ADR-091
tests:
- TC-470
- TC-471
- TC-472
- TC-473
domains: []
domains-acknowledged: {}
---

## Description

The cluster executes cells strictly sequentially, so wall time is the *sum* of cell latencies even though the `derived_from` graph is wide: after `rust_struct`, four cells (`shacl_shape`, `iri_module_consts`, `parser`, `emitter`) have no mutual edges, and the two test cells depend only on those. With SPMC bundles ([FT-177](FT-177)/[FT-178](FT-178)) cells converge in 1–2 turns, making LLM latency the dominant term — a clean run spends most of its 4–6 minutes waiting on one cell at a time.

This slice executes the topo graph **by level** with a bounded worker pool: all cells whose upstream dependencies are satisfied dispatch concurrently, capped at a configurable concurrency (default 3 — sized so a level's combined draw stays inside the 200k TPM window). Expected wall time for `add-artifact-type`: longest path of 3 levels ≈ **2–3 minutes**.

## Functional Specification

### Inputs

- `run_cells` / `emit_llm_cell` and the FT-171 repair loop in `crates/decision-cli/src/features/drive/cluster_dispatch.rs`.
- `topo_order` in `crates/dec-harness/src/task_type/topo.rs` — extended with a `topo_levels` grouping (cells per dependency depth).
- `.dec/task-types.toml`: optional `[concurrency]` table, `max_parallel_cells` (default 3, 1 ≡ today's sequential behaviour).

### Outputs

- `topo_levels(&[CellDecl]) -> Result<Vec<Vec<String>>, …>` in dec-harness: deterministic level grouping (cells sorted by name within a level).
- `run_cells` executes level-by-level: within a level, cells dispatch on scoped threads (`std::thread::scope`), at most `max_parallel_cells` in flight; the level completes when every cell in it has (with FT-171 placement retries applied per cell).
- `cell_sessions` and `cell_outputs` become mutex-guarded within the level scope; the FT-146 SessionRecord rows and FT-170 placement semantics are unchanged.
- FT-135 progress narration interleaves with `[cell]`-distinguishable lines (already prefixed by the worker passthrough).

### State

- No graph-resident changes.

### Behaviour

1. Levels execute in order; a cell failure (post-retries) aborts at the level boundary — cells already in flight in the same level run to completion (their session records persist), later levels never start.
2. The FT-171 audit-repair loop is level-aware: implicated cells re-dispatch concurrently when they share a level, sequentially across levels.
3. Mechanical cells execute inline (no thread) — they are instant.

### Invariants

- Output equivalence: for any TaskType, the post-run sandbox is byte-identical to a sequential run with the same worker outputs (placement paths are per-cell disjoint by registry guarantee).
- A cell never dispatches before every `derived_from` upstream has completed and its output is readable in `cell_outputs`.
- `max_parallel_cells = 1` reproduces today's behaviour exactly.

### Error handling

- A panicked cell thread is converted to a cell failure with the panic payload in the diagnostic (no poisoned-mutex propagation: lock poisoning maps to a cluster error naming the cell).

### Boundaries

- Concurrency is per-cluster; multi-feature fleet parallelism (several clusters at once) is the tier-upgrade conversation, out of scope.
- Worker contract unchanged.

## Out of scope

- Cross-level pipelining (starting a downstream cell before its whole level closes).
- Adaptive concurrency from rate-limit headers (revisit after the tier decision).