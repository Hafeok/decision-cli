---
id: FT-157
title: 'decision-cli: archetype-dispatch planner — CLASSIFY → PLAN → DISPATCH → AUDIT → ASSEMBLE → REPORT loop'
phase: 5
status: planned
depends-on:
- FT-153
- FT-154
- FT-155
adrs:
- ADR-082
- ADR-084
tests: []
domains:
- api
domains-acknowledged: {}
---

## Description

The archetype-dispatch planner: a new planner under `dec drive` that implements the six-step loop from `briefs/feature-authoring-brief.md §1`: **CLASSIFY → PLAN → DISPATCH → AUDIT → ASSEMBLE → REPORT**. Composes the classifier worker ([FT-155](FT-155)), the cluster-dispatch executor ([FT-139](FT-139), [FT-150](FT-150), [FT-151](FT-151)), the three-scope audit pipeline ([FT-153](FT-153)), and the escape-hatch routing ([FT-154](FT-154)) into a single dispatch path keyed on an archetype id.

This is the planner that makes the archetype layer operationally real. Without it, the substrate from FT-147..FT-156 exists but the CLI's `dec drive ship` path still routes everything through the v1 single-TaskType classifier; this slice adds the archetype-aware branch that walks the full loop.

Planner inherits the iterative-driver shape from [PAT-001](PAT-001) (Inspector + Planner trait pair) and [PAT-002](PAT-002) (state-hash ring buffer for cycle detection).

## Functional Specification

### Inputs

- The existing `dec drive` planner registry at `features/drive/planners/`.
- `Archetype` from [FT-147](FT-147), `TaskType` from FT-150, classifier verdicts from FT-155, audit pipeline from FT-153, escape-hatch routing from FT-154.
- The `Cluster::topo_order` utility from FT-139 + the `provisioning.depends_on` ordering from FT-151.
- `pyoxigraph` bundle store (in-memory) — the planner queries the bundle.

### Outputs

**New planner module** at `crates/decision-cli/src/features/drive/planners/archetype_dispatch.rs`:

```rust
pub struct ArchetypeDispatchPlanner;

impl Planner for ArchetypeDispatchPlanner {
    fn name(&self) -> &str { "archetype-dispatch" }

    fn applies(&self, args: &DriveArgs) -> bool {
        // Applies when the operator passes --archetype <id>, OR when the feature_spec
        // carries `archetype: <id>` in front-matter, OR when `dec drive archetype <id>` is invoked.
        args.archetype.is_some()
    }

    fn plan(&self, ctx: &DriveContext, args: &DriveArgs) -> PlanOutcome {
        // 1. CLASSIFY: dispatch the archetype-classifier worker, get verdicts per unit.
        let verdicts = dispatch_classifier(ctx, args)?;

        // 2. PLAN: split verdicts by family, build the dispatch ordering.
        let plan = build_dispatch_plan(verdicts, &archetype.task_types)?;

        // 3. DISPATCH: for each cluster in plan order, run cluster_dispatch.
        let mut outcomes = vec![];
        for cluster_id in plan.order {
            let outcome = match cluster_id.target {
                Target::Cluster(task_type_id) => cluster_dispatch::run(ctx, args, task_type_id)?,
                Target::Escape(unit) => broad_worker_dispatch::run(ctx, args, unit)?,
            };
            outcomes.push(outcome);
        }

        // 4. AUDIT: three-scope pipeline runs inside cluster_dispatch per FT-153 — this stage
        //    here aggregates outcomes and gates ASSEMBLE on full-cluster green.
        let audit_aggregate = aggregate_audit_results(outcomes)?;

        // 5. ASSEMBLE: place artifacts per ApplicationContract feature-organisation.
        let assembly = assemble_artifacts(audit_aggregate.outputs, archetype.application_contract)?;

        // 6. REPORT: emit ClusterReport into drive history.
        let report = build_cluster_report(verdicts, outcomes, audit_aggregate, assembly);
        ctx.drive_history.append(report);

        PlanOutcome::Done(plan)
    }
}
```

**Dispatch plan builder (`build_dispatch_plan`)**:

- Group verdicts by matched TaskType.
- Split TaskTypes by family: infrastructure-family first, application-family second.
- Within infrastructure-family: topological sort by `provisioning.depends_on` per FT-151.
- Within application-family: topological sort by inter-cluster `derived_from-contract` references (an application cluster reading a convention populated by another cluster lands after that other cluster).
- Escape-hatch units interleave per their unit position in the original request — they run on the broad worker but their assembly slot is preserved.
- Surface the plan to the operator before any dispatch with side effects begins (per brief §1.2: "Produce an ordered dispatch plan. Surface it before executing anything with side effects.").

**Operator gate on side-effecting dispatches:**

Per brief §4, infrastructure work touches live cloud state. Before any infrastructure-family TaskType dispatches:
- The planner runs a Bicep `what-if` (or the equivalent declarative-IaC dry-run).
- Surfaces the planned changes for operator approval.
- Aborts the entire drive if the operator does not approve (or if `--auto-approve-infra` is not set and the run is non-interactive).
- Once approved, dispatches the infrastructure TaskTypes in order.

**Assembly:**

- Reads `ApplicationContract.feature_organisation.body_path` to determine the layout rule.
- Places each cluster output at its conventional location (e.g., for the decision-cli archetype: `crates/decision-cli/src/features/ft_NNN_<title>/` for application clusters).
- The placement is reversible — every file write is logged so `dec drive abort` can roll back via worktree reset.

**Report:**

`ClusterReport` shape (logged into drive history):
- Units identified (count + per-unit summary).
- TaskTypes dispatched (with IRIs + versions if FT-150 grew versioning).
- Audit results per scope (per-type pass/fail, archetype pass/fail, seam pass/fail).
- Escape-hatch routing (units, broad-worker session IRIs, candidate IRIs emitted by FT-154).
- Contract pressure observed (forwarded from FT-155 verdicts).
- Assembly summary (files placed, locations).

Surfaced in `dec drive show` (existing renderer; extended for archetype-dispatch reports).

**Test coverage:**

- Positive end-to-end: a feature request with two units, both high-confidence, one application + one infrastructure TaskType. Plan dispatches infrastructure first, then application; audits pass at all three scopes; assembly places artifacts; report emitted with all sections populated.
- Plan ordering: feature with one infrastructure A, one infrastructure B where B.depends_on=A, two application clusters → assert A, then B, then applications.
- Escape-hatch in plan: one unit unmatched → plan includes a broad-worker step for that unit; rest of dispatch proceeds.
- Audit failure aborts assembly: synthetic seam-audit failure on cluster 2 → plan aborts; cluster 1's artifacts rolled back; report shows the failing audit; no commit.
- Infrastructure `what-if` gate: non-interactive run without `--auto-approve-infra` and an infrastructure dispatch needed → planner aborts with `RequiresApproval` outcome.
- Cycle detection: synthetic infrastructure TaskType set with a depends_on cycle → caught at plan time (already E118 at FT-151 SHACL; planner check is defensive).
- Cluster report: all sections render; matches the snapshot fixture.

### State

- **New on-disk:** `features/drive/planners/archetype_dispatch.rs`; planner registry update in `features/drive/planners/mod.rs`; `features/drive/cluster_report.rs` (ClusterReport type + renderer).
- **Modified on-disk:** `features/drive/run.rs` (planner registry wiring); `features/drive/show.rs` (archetype-dispatch report rendering).
- **CLI args:** new `--archetype <id>` flag; new `dec drive archetype <id>` subcommand (per FT-159 wiring).

### Behaviour

1. **Planner registered with the existing drive registry**. Applies when archetype flag/subcommand selected.
2. **Loop runs in strict order**: classifier first; plan surfaced before any side-effecting dispatch; dispatches in plan order; audits at three scopes via cluster_dispatch; assembly; report.
3. **Iterative-driver shape** (PAT-001 + PAT-002): state hashes recorded between steps; cycle detection guards against accidental re-entry; bounded loop count.
4. **Idempotent re-runs**: re-running `dec drive ship --archetype X FT-Y` against a feature whose clusters previously dispatched and audited green is a no-op (skip with `AlreadyShipped { feature }` outcome).

### Invariants

- **Plan surfaces before side effects**. Operator approval is required for infrastructure dispatches in non-interactive runs without `--auto-approve-infra`.
- **Audit pipeline is non-skippable**. Per-type → archetype → seam runs for every cluster; FT-153's fail-fast governs.
- **Assembly happens once, after all audits pass**. No partial assembly.
- **Escape-hatch outcomes are reported alongside cluster outcomes**. Broad-worker dispatches show up in the report on equal footing.
- **Reports are graph-resident**. Drive history is queryable; reports survive across runs.

### Error handling

- **Classifier worker dispatch failure** → planner aborts; nothing dispatched; report records the classifier failure.
- **Plan-time cycle detected** → aborts with `PlanCycleDetected { cycle }`; defensive check (also caught at SHACL E118).
- **Operator approval refused** → aborts with `OperatorAborted`; no dispatches; clean exit.
- **Cluster dispatch failure** → audit pipeline rollback semantics from FT-153; subsequent clusters in plan order are skipped; report shows the failure.
- **Assembly file-write failure** → worktree reset; report shows the assembly stage failure; cluster outputs still in the graph (audits passed) but uncommitted.

### Boundaries

- **In scope.** The archetype-dispatch planner; plan-builder logic; assembly; report; CLI wiring under `--archetype` and `dec drive archetype <id>` (the latter shared with FT-159's listing surface); seven TCs.
- **Out of scope.** `dec archetype list / show / promote / demote / reframe-instance` CLI verbs — FT-158. The actual broad-worker implementation — FT-123 (already exists). LLM-driven plan rewriting — humans-in-the-loop approve the plan. Parallel cluster dispatch — sequential for v1. Resume from partial-dispatch failure — re-run is the v1 path; resume is a future enhancement.

## Out of scope

- `dec archetype` CLI verbs (FT-158).
- Broad-worker implementation (FT-123 exists).
- LLM-driven plan rewriting.
- Parallel cluster dispatch.
- Partial-dispatch resume.
- Cross-archetype plans (a single feature spanning two archetypes).
