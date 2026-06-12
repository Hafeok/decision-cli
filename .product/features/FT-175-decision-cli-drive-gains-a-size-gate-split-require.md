---
id: FT-175
title: 'decision-cli: drive gains a size gate (split-required) and a splitter role for oversized features'
phase: 5
status: planned
depends-on:
- FT-131
adrs:
- ADR-089
- ADR-066
- ADR-036
- ADR-075
tests:
- TC-445
- TC-446
- TC-447
- TC-448
domains:
- api
- data-model
domains-acknowledged:
  ADR-083: Size thresholds bind as orchestration policy, not as archetype/instance/feature tech detail; no binding-level question arises.
  ADR-087: The size gate and splitter touch planning and spec authoring; no audit emission or repair targeting changes.
  ADR-084: No archetype ships or changes status; seam audits are unaffected by spec-layer decomposition.
  ADR-081: split-required renders inside existing drive show output; no new enumerate/lookup verb pair is introduced (dec drive split is an action verb).
  ADR-082: Splitting operates on feature_specs above the TaskType layer; archetype contracts and TaskType families are untouched.
---

## Description

Implements [ADR-089](ADR-089): the drive planner refuses to dispatch an oversized feature. Deterministic size signals (spec body length, functional-spec subsection count, linked-TC count, domain breadth) are computed as pure graph reads inside the DoR/ship chain ([FT-119](FT-119)/[FT-131](FT-131)); a signal over threshold makes the planner return a `split-required` stuck variant carrying the measurements. The remedy is spec-layer decomposition: a `splitter` role (graph-authoring worker per [ADR-066](ADR-066)) proposes child feature_specs with sibling `depends-on` edges and a parent umbrella link, staged for operator acceptance per [ADR-075](ADR-075). The concept adapts the pipeline factory's batch-splitting step (decompose before any expensive work) to the spec-first graph.

## Functional Specification

### Inputs

- The feature's spec body, linked TCs, and declared domains from the product graph (pure reads).
- A size-policy artifact in the orchestration store (thresholds); compiled defaults when absent, set below the [FT-163](FT-163) 50k framing cap.
- The splitter role's catalog entry: capability binding ([ADR-033](ADR-033)/[ADR-047](ADR-047)), `dec:roleTool` surface ([ADR-070](ADR-070)), bundle definition (parent spec + dependency context + sibling conventions).

### Outputs

- A `split-required` planner outcome (new Stuck variant) carrying each measured signal and its threshold; rendered by `dec drive show`.
- A staged split proposal: child feature_spec drafts, sibling `depends-on` edges, parent-umbrella linkage, TC stubs per child.
- On acceptance: children land in `.product/features/` via the product authoring surface; derivation provenance (split-from + triggering signals) recorded per [ADR-038](ADR-038).

### State

- New orchestration-store policy artifact for thresholds (graph-resident per [ADR-036](ADR-036)).
- Parent feature gains umbrella marking and child links; children are ordinary features thereafter.

### Behaviour

1. The size gate runs after DoR completeness checks, before any implementation dispatch; under-threshold features flow through unchanged.
2. `split-required` halts the feature's round; in `dec drive ship --all` sweeps the failure is isolated per [FT-111](FT-111) (other features continue).
3. The splitter dispatch is operator-initiated in this slice (`dec drive split FT-XXX`); its proposal is staged, never auto-applied.
4. Accepted children re-enter the normal pipeline: DoR, preflight, `product verify`, dependency-ordered dispatch.

### Invariants

- Oversize detection consults no LLM and reads only the graph — identical inputs give identical gate outcomes.
- A split never bypasses gates: every child passes DoR and preflight independently before dispatch.
- The parent is never deleted; identity and history survive the split.

### Error handling

- Splitter proposals failing structural validation (`product graph check` on the staged proposal, missing required body sections, dangling depends-on) are rejected with the diagnostic; nothing lands in the product graph.
- A missing/invalid policy artifact degrades to compiled defaults with a warning, never to an open gate.

### Boundaries

- Acceptance autonomy is ADR-075's: this slice does not auto-accept split proposals.
- The size gate measures the spec, not the diff — PR-size enforcement (the factory's 800-line build gate) is a separate concern for the verify pipeline, out of scope here.

## Out of scope

- Auto-acceptance or graduation of splitter proposals.
- Recursive splitting (children exceeding thresholds surface `split-required` again on their own rounds; no recursion inside one round).
- Retro-splitting features already in progress or complete.
- Cluster-cell-level decomposition (cells are typed by TaskType, not sized by spec — ADR-089 rejected alternative).
