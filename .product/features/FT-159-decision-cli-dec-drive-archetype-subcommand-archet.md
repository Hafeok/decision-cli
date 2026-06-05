---
id: FT-159
title: 'decision-cli: dec drive archetype subcommand — archetype-keyed feature dispatch driver'
phase: 5
status: planned
depends-on:
- FT-157
adrs:
- ADR-011
tests: []
domains:
- api
domains-acknowledged: {}
---

## Description

The `dec drive archetype <archetype-id> <feature-id>` subcommand — the archetype-keyed entrypoint to the dispatch loop from [FT-157](FT-157). Convenience wrapper around `dec drive ship --archetype <archetype-id> <feature-id>` plus the multi-feature sweep mode (`dec drive archetype <archetype-id> --all`) analogous to [FT-111](FT-111).

This is the operator's primary verb for archetype-driven feature shipping. Without it, operators have to invoke `dec drive ship --archetype X FT-Y` for every feature; with it, the archetype is the noun and the action is implicit (ship). Mirrors the shape of [FT-110](FT-110) (`dec drive ship`) and [FT-111](FT-111) (`dec drive ship --all`).

## Functional Specification

### Inputs

- `Archetype` from [FT-147](FT-147).
- The archetype-dispatch planner from FT-157.
- The existing `dec drive` planner registry.
- The feature graph (target archetype's features queryable by `archetype` link or front-matter `archetype: <id>` field).

### Outputs

**Clap subcommand** under `dec drive`:

```
dec drive archetype <archetype-id> <feature-id>                  # single-feature ship
dec drive archetype <archetype-id> --all                          # sweep every shippable feature in the archetype
dec drive archetype <archetype-id> --auto-approve-infra           # skip the IaC what-if approval gate
dec drive archetype <archetype-id> --dry-run                      # plan only; surface plan; do not dispatch
dec drive archetype <archetype-id> --include-feedback             # respect FT-138 open-implementer-feedback gate
```

**Handler at `crates/decision-cli/src/features/drive/cli_archetype.rs`:**

1. Look up the archetype; refuse with diagnostic if not found.
2. Refuse with diagnostic if `Archetype.status: quarantined`.
3. Single-feature mode: resolve `<feature-id>`; assert it is bound to `<archetype-id>` (via FT-150 archetype back-reference); invoke the FT-157 planner with `args.archetype = Some(<id>)`.
4. Sweep mode (`--all`): enumerate features bound to the archetype with `status != complete`; run the def-ready planner ([FT-119](FT-119)) per feature to filter to ready ones; dispatch each via FT-157 planner; aggregate outcomes into a per-feature tally (mirrors FT-111's tally shape).
5. Dry-run mode: invoke the FT-157 planner up through the PLAN step; surface the dispatch plan; abort without dispatching.

**Sweep tally:**

Per [PAT-003](PAT-003) (multi-artifact sweep with per-item bounded execution and structured tally):

```rust
struct ArchetypeSweepReport {
    archetype: String,
    total_features: usize,
    shipped: Vec<FeatureId>,
    not_ready: Vec<(FeatureId, NotReadyReason)>,
    failed: Vec<(FeatureId, FailureReason)>,
    elapsed: Duration,
}
```

Surfaces in `dec drive show` and as CLI output. Mirrors FT-111's report shape.

**Refusal modes:**

- Archetype `quarantined` → `ArchetypeQuarantined { archetype, reason }` outcome.
- Feature not bound to archetype → `FeatureNotInArchetype { feature, archetype }` outcome.
- Feature `status: complete` → informational skip in sweep mode; error in single-feature mode.
- Operator approval refused (interactive run) → `OperatorAborted` per FT-157.
- Per-feature failures in sweep mode → tally records the failure; sweep continues with the next feature.

**Test coverage:**

- Single feature, happy path: feature bound to archetype, classifier matches, audits pass, assembly succeeds; outcome `Shipped`.
- Single feature, not bound to archetype: refusal with diagnostic.
- Single feature, archetype quarantined: refusal.
- Sweep with two features, both ready: both ship; tally counts 2 shipped.
- Sweep with one ready + one not-ready (open implementer feedback per FT-138): tally counts 1 shipped, 1 not-ready.
- Sweep with one ready + one failing seam audit: tally counts 1 shipped, 1 failed; failure detail records the seam-audit identifier.
- Dry-run mode: plan emitted to stdout; no clusters dispatched; no commit.
- `--auto-approve-infra` skips the operator approval gate for an infrastructure-family TaskType.

### State

- **New on-disk:** `features/drive/cli_archetype.rs` (handler), `features/drive/archetype_sweep.rs` (sweep logic + tally type).
- **Modified on-disk:** `main.rs` (clap registration), `features/drive/show.rs` (archetype-sweep report rendering).

### Behaviour

1. **Cluster dispatch via `add-cli-subcommand`** ([FT-142](FT-142)).
2. **Single-feature mode is a thin wrapper around FT-157**. Convenience shape; no new dispatch logic.
3. **Sweep mode reuses FT-111's per-item bounded execution pattern**. Each feature gets its own dispatch with its own audits.
4. **Dry-run gates at PLAN**. The FT-157 planner supports a dry-run flag that returns the plan without dispatching; this handler surfaces it.

### Invariants

- **No silent cross-archetype dispatches.** Feature must be bound to the archetype passed in; refusal otherwise.
- **Sweep is per-feature isolated.** A failing feature does not abort the sweep; the tally records the failure and the sweep continues.
- **Dry-run never has side effects.** Worktree, graph, and infrastructure all untouched.

### Error handling

- **Archetype not found** → `ArchetypeNotFound { id }`.
- **Archetype quarantined** → `ArchetypeQuarantined { reason }`.
- **Feature not bound to archetype** → `FeatureNotInArchetype`.
- **Feature status complete** → informational in sweep; error in single mode.
- **FT-157 plan-time errors** propagate per its existing error types.

### Boundaries

- **In scope.** The `dec drive archetype <archetype-id>` subcommand; single-feature + sweep + dry-run modes; tally report rendering; eight TCs.
- **Out of scope.** `dec drive archetype extract` (the pattern-extractor entrypoint) — possible future verb. Multi-archetype sweep (sweep across two archetypes) — out of v1. Parallel feature dispatch — sequential per FT-157.

## Out of scope

- Pattern-extractor CLI verb (future).
- Multi-archetype sweep.
- Parallel dispatch.
- LLM-driven sweep planning.
