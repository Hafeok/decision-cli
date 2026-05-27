---
id: FT-110
title: 'decision-cli: dec drive — pluggable artifact+goal orchestrator with FT+Ship planner'
phase: 3
status: complete
depends-on:
- FT-099
- FT-107
- FT-108
- FT-109
adrs:
- ADR-004
- ADR-030
tests:
- TC-195
- TC-196
- TC-197
- TC-198
- TC-199
domains: []
domains-acknowledged: {}
---

## Description

The verify → re-fix loop is now wired piecewise: [FT-099](FT-099)'s `dec verify feature` runs all the graphs, [FT-107](FT-107)'s `dec verify graph generate` re-authors broken graphs from defect feedback, [FT-108](FT-108)'s `dec implement` consumes implementer-targeted defects, and [FT-109](FT-109)'s `dec loop show / list` reports the state. What's missing is the *outer driver* — the thing that chooses which sub-command to run next based on the artifact's current graph state and the goal the operator wants to reach.

This slice adds `dec drive <goal> <artifact>` as the outer loop, and — critically — does so via a **pluggable planner pattern** so future goals (`dec drive accept ADR-XXX`, `dec drive cover TC-XXX`) drop into the same machinery without rewriting the driver.

The architecture mirrors the harness loop described in [`docs/ddd/Implementing_DDD.md`](docs/ddd/Implementing_DDD.md): **the graph is the state machine, planners are state-classifiers, the driver is a tight execute-and-retry shell**. The slice-1 simplification is that planners are hardcoded Rust impls instead of being looked up from a `dec:DispatchRule` graph artifact — but the shape is forward-compatible with that.

One subcommand → one slice — `dec drive` is the new verb, the FT+Ship planner is the first concrete impl, and the surrounding substrate (`ArtifactRef`, `Planner` trait, `Action` enum, registry) is the substrate every future planner reuses. Together they're tight enough for one slice.

## Functional Specification

### Inputs

#### `dec drive <goal> <artifact>`

```
dec drive ship FT-XXX [--max-iter N] [--env ENV-NNN]
```

- `goal` — one of `ship`, `verify`, `accept`, `cover`, `approve`. Maps to the `Goal` enum.
- `artifact` — the artifact's short id. The prefix determines the `ArtifactKind`: `FT-` / `TC-` / `VG-` / `ENV-` / `ADR-`.
- `--max-iter N` — bail out after N planner cycles (default 5). Prevents runaway worker spend when the loop doesn't converge.
- `--env ENV-NNN` — environment id for verify dispatches that need one. Falls back to the planner's default-env lookup when omitted.

#### Substrate

The reusable surface lives under `core::drive::`:

```rust
pub struct ArtifactRef { pub kind: ArtifactKind, pub short_id: String }
pub enum ArtifactKind { Feature, TestCriterion, VerificationGraph, Environment, Adr }
pub enum Goal { Ship, Verify, Accept, Cover, Approve }

pub enum Action {
    Done,
    DispatchVerifier { feature_id: String, env_id: String },
    DispatchImplementer { feature_id: String },
    DispatchVerifyGraphAuthor { feature_id: String, env_id: String },
    Stuck { reason: String },
}

pub trait Planner {
    fn plan(&self, ctx: &PlanContext, artifact: &ArtifactRef) -> Result<Action>;
}

pub fn planner_for(kind: ArtifactKind, goal: Goal) -> Option<Box<dyn Planner>>;
```

`PlanContext` carries the orchestration store handle plus pre-resolved metadata (workdir, product_root, default env). Planners compose against shared read primitives — they don't re-implement the verdict aggregator, the defect-feedback loader, etc. Those already exist in core and the FT-107/108 features.

### The FT+Ship planner

```rust
impl Planner for FeatureShipPlanner {
    fn plan(&self, ctx, ft) -> Result<Action> {
        let verdict = ctx.aggregate_verdict_for_feature(&ft.short_id)?;
        let impl_open = ctx.open_defect_feedback(&ft.short_id, "implementer")?.len();
        let vga_open  = ctx.open_defect_feedback(&ft.short_id, "verifier")?.len();
        match (verdict, impl_open > 0, vga_open > 0) {
            (Approved, _, _)               => Action::Done,
            (_, true, _)                   => Action::DispatchImplementer { ... },
            (_, _, true)                   => Action::DispatchVerifyGraphAuthor { ... },
            (NeverRun, false, false)       => Action::DispatchVerifier { ... },
            (Rejected | Amendment, false, false)
                                           => Action::Stuck { reason: "feedback addressed but verify still failing; \
                                                                       worker not converging" },
        }
    }
}
```

### Outputs

- **`Done`** — driver exits 0 with `DriveOutcome::Reached { iterations, history }`. The history is a list of every action that fired so post-mortem audit is trivial.
- **`Stuck`** — driver exits non-zero with a renderable reason. Common case: the worker addressed every cited feedback but verify still fails, which means the worker isn't producing real fixes (model quality issue, not framework bug).
- **`MaxIterations`** — driver exits non-zero with the iteration count + history. Indicates the loop is making progress but slowly, or oscillating.

Text rendering by default; `--format json` for piping the outcome into downstream tools.

### State

- No on-disk schema change.
- Reads: aggregate verdict (FT-099), defect feedback (FT-107/108 loaders), feature-spec metadata.
- Writes: nothing direct — every persisted side-effect is the responsibility of the dispatched handler (`verify_feature::run`, `implement::run`, `verify_graph_generate::run_generate`). The driver is a coordinator, not a writer.

### Behaviour

1. Parse `<artifact>` → `ArtifactRef`.
2. Look up `planner_for(kind, goal)`. `None` → exit with "no plan registered for (kind, goal)".
3. Open `PlanContext` against the working tree.
4. Loop up to `max_iter`:
    a. `planner.plan(ctx, artifact)` → `Action`.
    b. Append to history.
    c. If `Done` → return `Reached`.
    d. If `Stuck` → return `Err(StuckErr)`.
    e. Otherwise dispatch the action by calling the relevant feature handler.
5. Loop body exited without `Done` → return `Err(MaxIterations)`.

### Error handling

- Unknown artifact prefix → `Err::InvalidArgument` with the list of supported prefixes.
- No planner for `(kind, goal)` → `Err::NoPlannerRegistered { kind, goal }`. Includes a hint about which combinations *are* registered so the operator knows what's supported today.
- A dispatched action fails (worker subprocess crashes, validator refuses) → the failure propagates; the driver does NOT swallow it. The history captures the action that errored.

### Out of scope

- ADR+Accept, TC+Cover, VG+Approve planners. The substrate accepts them, the slice doesn't ship them. Each is a follow-up of ~50 lines.
- `--watch` mode that polls `dec loop list` and drives features with open defects automatically. The interactive `dec drive FT-XXX` is the primitive; the watcher is a wrapper.
- DispatchRule artifacts in the graph (the DDD-native form of the registry). Slice-1 hardcodes planners in Rust; the trait shape is forward-compatible with graph-resident rules.

## Acceptance

1. `ArtifactRef::parse` returns the right `ArtifactKind` for every supported prefix and `InvalidArgument` for unknown ones.
2. `FeatureShipPlanner::plan` returns the correct `Action` for every cell in the (verdict × impl-open × vga-open) classification table (see TC-196).
3. `drive::run` respects `max_iter` and surfaces a `Stuck` reason verbatim when a planner returns one.
4. `drive::run` terminates with `DriveOutcome::Reached` on the first iteration that the planner reports `Done`, with `iterations == 0` for already-shipped features.

## Notes

The reusability bet pays off the first time we add a second planner. ADR+Accept will be ~50 lines (one trait impl + a state classifier that reads ADR-status edges). TC+Cover even less. After three or four planners, the trait surface itself becomes a candidate for graph-residency — `dec:DispatchRule` artifacts whose body is JSON Logic or a small DSL, looked up at runtime. That's a FT-12X conversation; today's slice-1 hardcoded planners are the operationally correct first step.
