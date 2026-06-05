---
id: FT-164
title: 'decision-cli: cluster_dispatch max_turns cap bumped from 8 to 40 as cost guardrail'
phase: 4
status: complete
depends-on:
- FT-139
- FT-163
adrs:
- ADR-080
tests:
- TC-399
- TC-400
- TC-401
- TC-402
domains:
- api
domains-acknowledged: {}
---

## Description

Sibling to [FT-163](FT-163). Where FT-163 bumped the per-cell *spec framing* cap, this slice bumps the per-cell *agentic-loop turn* cap and makes it configurable per task type via `.dec/task-types.toml`.

Witnessed by FT-147's retried dispatches against the new framing window: `rust_struct` cell produces shape-correct output (the FT-163 win), but downstream cells (`emitter`, `round_trip_tests`) intermittently fail with "did not produce <file>" because the worker's `for turn in range(max_turns)` loop ([code-writer/agent/loop.py](workers/code-writer/src/code_writer/agent/loop.py)) exhausts its budget while emitting substantial Rust before reaching the final `write_file` call. Empirically: emitter cell consumes 37k input / 5.2k output tokens when it succeeds — it has work to do.

The 8-turn cap dates to the FT-139 prototype when cells were small. It was set as a *cost safety net*, not a tuning dial — the goal is to catch a stuck model burning compute, not to limit legitimate work. A turn cap that aborts ~30% of cluster dispatches is the wrong-shape safety: the operator pays for the partial dispatch *and* retries, often more expensive than letting the original run to completion.

**40 turns at Scaleway qwen3-coder rates ≈ €0.25 per cell maximum** (output-token dominated). Cross-multiplied: a 6-cell cluster has ≈ €1.50 worst-case exposure to a single stuck cell. Still solidly in "dimes territory" — the safety net catches truly pathological loops without strangling normal substrate work.

Config-driven, not hardcoded. The default lives as a module constant; per-task-type overrides land in the existing `.dec/task-types.toml` next to the routing table. Different task types have different turn-budget needs — judge clusters are small (5-cell, mostly Python boilerplate); artifact-type clusters are bigger (6-cell, substantial Rust per cell). One blanket cap is the wrong shape; per-task-type config tracks reality.

## Functional Specification

### Inputs

- `crates/decision-cli/src/features/drive/cluster_dispatch.rs::emit_llm_cell` — the call site that sets `max_turns: 8` on each cell's `DispatchPayloadJson`.
- `.dec/task-types.toml` — existing routing config; this slice adds a sibling `[task_types.<name>]` table for cluster dispatch overrides.
- The witnessed FT-147 dispatches (€0.07 across 3 retries, 50% of cells timed out before reaching `write_file`).

### Outputs

**Module constant + config surface** in `cluster_dispatch.rs`:

```rust
/// Default per-cell agentic-loop turn cap. Catalog overrides
/// (see .dec/task-types.toml [task_types.<name>] max_turns) take precedence.
const MAX_CELL_TURNS: u32 = 40;
```

**TOML override schema** (additive — file works fine without any overrides):

```toml
# .dec/task-types.toml
[features]
"FT-147" = "add-artifact-type"

# FT-164: per-task-type cluster dispatch overrides.
# Each table is optional. Fields absent from the table fall back to the
# module constant default in cluster_dispatch.rs.
[task_types.add-artifact-type]
max_turns = 40
```

**Lookup helper** at `crates/decision-cli/src/features/drive/planners/feature_ship.rs` (or co-located with `read_task_type_from_routing_config`):

```rust
fn read_max_turns_for_task_type(cwd: &Path, task_type_name: &str) -> Option<u32>
```

Returns the override when `[task_types.<name>] max_turns` is set, `None` otherwise. Defensive — any IO / parse error degrades to `None` so the caller falls back to the const.

**Wiring** at `emit_llm_cell`: resolve the override (via `ctx.workdir`), fall back to `MAX_CELL_TURNS` const. One-line change at the payload construction site.

### State

- **Modified on-disk:** `crates/decision-cli/src/features/drive/cluster_dispatch.rs` — const, helper-call, docstring. `crates/decision-cli/src/features/drive/planners/feature_ship.rs` — new helper `read_max_turns_for_task_type`.
- **Convention on-disk:** `.dec/task-types.toml` gains an optional `[task_types.<name>]` table convention. No file rename — the existing path keeps working with or without the new table.

### Behaviour

1. **Default**: every LLM-backed cell dispatched through `emit_llm_cell` receives `max_turns: 40` (was 8) unless a per-task-type override is present in `.dec/task-types.toml`.
2. **Override**: when `[task_types.<name>] max_turns` is present in the routing config, that value is used for cells of TaskType `<name>`. Per-cell overrides are NOT supported in v1 (one knob per task type).
3. **Mechanical cells unaffected** — `max_turns` is only in the LLM payload.
4. **No cluster-execution semantics change** — same audit path, same fail-fast, same FT-146 SessionRecord persistence.
5. **Defensive degradation** — malformed TOML, missing file, missing table → fall back to the const. The dispatch never errors over a misconfigured override.

### Invariants

- **The default cap is u32, lives in source.** Operators set it deliberately when they ship a new release.
- **Override takes precedence over default.** When set, the TOML value is authoritative for cells of that task type.
- **No regression** — features that completed at the 8-cap (FT-145's add-cli-subcommand cluster) complete identically at the new default. Higher cap only matters when the model needs more turns.
- **Cost-safety property preserved**. Worst-case exposure per cell = output-token cap × cost rate × max_turns ≈ €0.25 on Scaleway at the default. Operators raising the override accept the corresponding higher worst-case.
- **Config absence is the no-op** — `.dec/task-types.toml` without a `[task_types.*]` table behaves identically to today.

### Error handling

- **Cell still exhausts the (resolved) cap** → existing `WorkerError(category="timeout")` path, unchanged.
- **TOML parse / IO error on the override read** → fall back to const, do not surface as a dispatch error.
- **Cap value out of range (negative, non-integer)** → fall back to const, log a tracing warning.

### Boundaries

- **In scope.** Const bump + TOML override surface + lookup helper + wiring in `emit_llm_cell` + 4 TCs.
- **Out of scope.** Per-cell overrides (e.g. `[task_types.add-artifact-type.cells.emitter] max_turns = 60`) — possible follow-on if witnessed need arises. Distinct `CellStatus::TimedOut` variant + persisted `dec:turnCount` predicate — observability slice deferred. CLI flag override (`dec drive ship --max-turns 60`) — operators set the catalog config, not per-dispatch ephemeral flags. Adaptive caps from historical telemetry. Per-cell budget enforcement based on cumulative cost rather than turn count.

## Out of scope

- Per-cell overrides.
- Distinct timeout cell status + new SessionRecord predicates.
- CLI flag override.
- Adaptive caps.
- Per-cell budget enforcement.
