---
id: PAT-001
title: Inspector and Planner trait pair for iterative drivers
status: live
domains:
- api
- data-model
adrs: []
requires: []
examples:
- FT-110
- FT-111
- FT-113
- FT-116
- FT-119
---

## When to use

Any driver loop that observes graph state, classifies it into a
next action, and dispatches — and where you want the
classification rule unit-testable without spinning up the real
orchestration store. The shape applies to `dec drive ship FT-XXX`,
will apply to `dec drive ship --all`, and would apply to any
future `dec drive ...` planner over a different goal axis.

The pattern's payoff is structural: the rule under test becomes
a pure function from `(verdict, open-feedback-counts,
graphs-exist)` to `Action`, exercisable in microseconds against
a hand-built stub. Mixing graph reads into the planner ties the
rule to a live store and forces every classification test to
seed real Turtle into Oxigraph.

## Prerequisites

- Familiarity with the `Action` enum in
  `crates/decision-cli/src/core/drive/action.rs` — the set of
  outcomes the driver's executor branches on.
- Familiarity with `PlanContext` and the existing
  `core::drive::Planner` trait — the driver passes one of these
  in to `plan(ctx, artifact)` on each iteration.

## The pattern

Two traits, one classifier function, two implementations of the
inspector — `Production` for real runs, `Stub` (or
`MutableStub`) for tests.

```rust
// crates/decision-cli/src/features/drive/inspect.rs — Inspector
pub trait GraphInspector {
    fn aggregate_verdict_for_feature(
        &self, feature_id: &str,
    ) -> Result<FeatureVerdict, InspectError>;

    fn open_defect_feedback_count(
        &self, feature_id: &str, role_id: &str,
    ) -> Result<usize, InspectError>;

    fn graphs_exist_for_feature(
        &self, feature_id: &str,
    ) -> Result<bool, InspectError>;

    // ...one method per dimension of observable state the planner reads.
}

// Production inspector reads from the real store + on-disk .ttl.
pub struct ProductionInspector<'a> { ctx: &'a PlanContext }
impl<'a> GraphInspector for ProductionInspector<'a> { /* SPARQL + fs */ }

// crates/decision-cli/src/features/drive/planners/feature_ship.rs — Planner
pub struct FeatureShipPlanner<I: GraphInspector> { inspector: I, /* state */ }

impl<I: GraphInspector> FeatureShipPlanner<I> {
    // Pure classification: reads inspector outputs, returns Action.
    // No I/O of its own; testable against a stub.
    pub fn classify(
        &self, feature_id: &str, env_id: &str,
    ) -> Result<Action, PlanError> {
        let verdict = self.inspector.aggregate_verdict_for_feature(feature_id)?;
        let impl_open = self.inspector.open_defect_feedback_count(feature_id, "implementer")?;
        // ...
        let intended = match (verdict, impl_open > 0, vga_open > 0) {
            (Approved, _, _) => Action::Done,
            (_, true, _)     => Action::DispatchImplementer { /* ... */ },
            // ...the classification table is the rule under test.
        };
        Ok(intended)
    }
}

impl<I: GraphInspector> Planner for FeatureShipPlanner<I> {
    fn plan(&self, ctx: &PlanContext, artifact: &ArtifactRef) -> Result<Action, PlanError> {
        self.classify(&artifact.short_id, &ctx.env_or_default("ENV-002"))
    }
}
```

Unit tests build a stub:

```rust
struct StubInspector { verdict: FeatureVerdict, impl_count: usize, vga_count: usize }
impl GraphInspector for StubInspector { /* return fixed values */ }

#[test]
fn implementer_open_dispatches_implementer() {
    let p = FeatureShipPlanner::new(StubInspector { /* ... */ });
    assert!(matches!(p.classify("FT-T", "ENV-002").unwrap(),
                     Action::DispatchImplementer { .. }));
}
```

A `MutableStubInspector` (using `Cell` for the dimensions) lets
tests model state evolving across calls — used for the
convergence-detection tests where the planner reads twice and
should react to a count that did or didn't drop.

## Anti-patterns

- **Reading from `PlanContext` (or any concrete store) inside the
  planner.** The classification rule becomes test-only-reachable
  via "seed Turtle, write to disk, load store, call plan." Every
  one-line table change costs an order of magnitude more test
  setup. Always go through the inspector trait.
- **Splitting the rule across `classify()` and the `Planner`
  trait impl.** Tests should be able to call `classify()`
  directly; `Planner::plan` is a one-line adapter that resolves
  env from `ctx` and calls `classify`. Putting any policy in
  `plan` makes it unreachable without `PlanContext::for_test`
  scaffolding.
- **One inspector method per SPARQL query instead of one per
  state dimension.** The trait is the contract between rule and
  store; rename SPARQL freely, but if `classify` reads
  `verdict`, `impl_open`, `vga_open`, the trait should have one
  method per — not `run_query_a`, `run_query_b`.

## Worked example

`FeatureShipPlanner` in
`crates/decision-cli/src/features/drive/planners/feature_ship.rs`
(FT-110). The classifier table is twelve lines; the planner has
24 unit tests in the same file plus TC-196's integration test,
all of which run in milliseconds against `StubInspector` /
`MutableStubInspector`. `ProductionInspector` lives next door in
`features/drive/inspect.rs` and is the only thing that knows
about Oxigraph, `.dec/verify/graph/*.ttl`, or
`load_store_from_dump`.

The cycle-detection backstop (PAT-002) hooks into the same
inspector trait via `state_hash_for_feature` — adding a state
dimension to a planner becomes "add a trait method, implement on
Production, implement on Stub, consume in `classify`." No
test infrastructure changes.
