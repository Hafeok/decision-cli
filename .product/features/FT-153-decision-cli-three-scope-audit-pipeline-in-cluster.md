---
id: FT-153
title: 'decision-cli: Three-scope audit pipeline in cluster_dispatch — per-type → archetype → seam, fail-fast'
phase: 5
status: planned
depends-on:
- FT-150
- FT-151
- FT-152
adrs:
- ADR-084
tests: []
domains:
- api
- observability
domains-acknowledged: {}
---

## Description

Extends [FT-139](FT-139)'s `cluster_dispatch` executor to run a **three-scope audit pipeline** at the assembly stage: per-type coherence (existing from FT-139) → archetype conformance (new — from [FT-152](FT-152)'s ArchetypeAudits) → seam audit (new — from FT-152's SeamAudits). Fail-fast at each stage; any failure rolls back the worktree.

This is the dispatch-time enforcement that makes [ADR-084](ADR-084)'s seam-audit mandate real. Without this pipeline, SeamAudits exist as artifacts but nothing runs them at dispatch time and the gate fails open. The pipeline is also the mechanism that propagates `safely_dispatchable: false` (from [FT-150](FT-150)) and `monolith_bar: CandidateAuditWeak` (from FT-152) into actual dispatch refusal: a TaskType bound to a weak-audit convention or an archetype with weak seam audits routes to the broad-worker escape hatch ([FT-154](FT-154)) instead.

## Functional Specification

### Inputs

- The existing `cluster_dispatch::run` at `features/drive/cluster_dispatch.rs` (post-FT-150 graph-resident TaskType lookup).
- `SeamAudit`, `ArchetypeAudit`, `RegressionEvidence`, `audit::run_audit` from [FT-152](FT-152).
- TaskType's `safely_dispatchable` field from FT-150.
- The per-type CoherenceAudit invocation from FT-139.
- `Archetype` from [FT-147](FT-147) — looked up for the dispatched TaskType to find its audit sets.

### Outputs

**Audit pipeline at `features/drive/cluster_dispatch.rs`:**

After all cells in a cluster have emitted their artifacts (the existing FT-139 + FT-150 path), invoke the three-scope pipeline before assembly:

```rust
fn run_audit_pipeline(
    cluster_outputs: &[CellOutput],
    task_type: &TaskType,
    archetype: &Archetype,
) -> Result<(), ClusterAuditFailure> {
    // Scope 1: per-type coherence (existing from FT-139)
    run_coherence_audit(task_type.coherence_audit, cluster_outputs)
        .map_err(|e| ClusterAuditFailure::PerType { audit: task_type.coherence_audit, detail: e })?;

    // Scope 2: archetype conformance
    for archetype_audit_iri in &archetype.archetype_audits {
        let audit = read_archetype_audit(archetype_audit_iri)?;
        if matches!(audit.monolith_bar, MonolithBar::Unrunnable) { continue; } // unrunnable audits skip with warning
        match audit::run_audit(audit) {
            AuditOutcome::Passed => continue,
            AuditOutcome::Failed { stderr } => return Err(ClusterAuditFailure::Archetype { audit: audit.id, stderr }),
            AuditOutcome::Unrunnable { stderr } => return Err(ClusterAuditFailure::ArchetypeAuditUnrunnable { audit: audit.id, stderr }),
        }
    }

    // Scope 3: seam audit — load-bearing per ADR-084
    for seam_audit_iri in &archetype.seam_audits {
        let audit = read_seam_audit(seam_audit_iri)?;
        if matches!(audit.monolith_bar, MonolithBar::Unrunnable) {
            return Err(ClusterAuditFailure::SeamAuditUnrunnable { audit: audit.id });
        }
        match audit::run_audit(audit) {
            AuditOutcome::Passed => continue,
            AuditOutcome::Failed { stderr } => return Err(ClusterAuditFailure::Seam { audit: audit.id, family: audit.family, stderr }),
            AuditOutcome::Unrunnable { stderr } => return Err(ClusterAuditFailure::SeamAuditUnrunnable { audit: audit.id }),
        }
    }

    Ok(())
}
```

Wired into `cluster_dispatch::run` between the cells-emitted-everything point and the assembly-stages-the-worktree point.

**Pre-dispatch refusal for weak-audit TaskTypes:**

Before the cluster's first cell dispatches, check:
- `task_type.safely_dispatchable == false` → refuse with `ClusterDispatchError::TaskTypeNotSafelyDispatchable { task_type, reason: "conforms_to weak-audit convention" }` and route to the broad-worker escape hatch.
- For infrastructure-family TaskTypes, additionally: if any of the archetype's seam audits is `monolith_bar: CandidateAuditWeak` AND that audit's family covers a slot this TaskType writes to → refuse with `ClusterDispatchError::WeakSeamCoverage { task_type, weak_audit }`.

Application-family TaskTypes are dispatchable even with weak seam audits — the seam-audit failure shows up after dispatch (at the seam-audit stage of the pipeline), which is the correct failure mode (the dispatcher tried, the audit caught it).

**Assembly stage:**

After audits pass, assemble: place cluster outputs in their conventional locations per the ApplicationContract's `feature_organisation` convention. The placement logic reads from FT-148's Convention.body_path content (which states the layout rule, e.g., "vertical slices: one folder per feature under `crates/decision-cli/src/features/ft_NNN_<title>/`").

The assembly step is reversible: every file write is logged; on later failure (or `dec drive abort`), the worktree is rolled back via `git worktree reset`.

**Report stage:**

After assembly + finalize, emit a `ClusterReport` with: units identified, TaskTypes dispatched (with versions), audit results per scope, anything routed to the escape hatch, any contract-pressure observed. Surfaced in `dec drive show` output. Saved as a `dec:ClusterRun` graph-resident record (an existing artifact type if present; otherwise a future slice — for v1, log to drive history).

**Test coverage:**

- Positive: end-to-end cluster dispatch where per-type, archetype, and seam audits all pass; assembly succeeds; report emitted.
- Negative (per-type audit fails): pipeline aborts at scope 1; archetype + seam audits not invoked; worktree rolled back; outcome surfaces the per-type audit identifier.
- Negative (archetype audit fails): pipeline aborts at scope 2; seam audit not invoked; outcome surfaces the failing ArchetypeAudit.
- Negative (seam audit fails): pipeline aborts at scope 3; outcome surfaces the SeamAudit + its family.
- Negative (`safely_dispatchable: false` TaskType): pre-dispatch refusal; broad-worker escape hatch dispatches instead.
- Negative (seam audit `Unrunnable`): pipeline aborts at scope 3 immediately; outcome distinct from `Failed`.
- Fail-fast ordering test: with all three audit scopes set to fail, the pipeline reports only the scope-1 failure (not all three).
- Application-family with weak seam audit: dispatch allowed; weak seam audit runs at scope 3; failure surfaces normally.
- Infrastructure-family with weak seam audit covering its slot: pre-dispatch refusal.

### State

- **Modified on-disk:** `features/drive/cluster_dispatch.rs` (audit pipeline + pre-dispatch refusal logic), `features/drive/run.rs` (report emission wiring), the existing `ClusterOutcome` enum gains the new failure variants.
- **No new artifact types** — orchestrates existing ones.

### Behaviour

1. **Pre-dispatch refusal**. `safely_dispatchable: false` TaskTypes never start; broad worker takes over via FT-154.
2. **Three-scope pipeline runs after cells emit, before assembly**. Fail-fast within and across scopes.
3. **Worktree rollback on any audit failure**. Existing FT-139 rollback path; extended to handle the new failure variants.
4. **Application-family vs infrastructure-family seam-audit semantics**. Application can dispatch under a weak seam audit (failure caught downstream); infrastructure cannot (failure too costly to risk).
5. **Report stage**. ClusterReport saved to drive history; surfaces in `dec drive show`.

### Invariants

- **No assembly without all three scopes passing.** Per-type → archetype → seam — fail at any scope, worktree rolls back, no commit.
- **Pre-dispatch refusal is loud.** Weak-audit dispatch refusal surfaces in drive history with the audit identifier; operator sees the diagnostic immediately.
- **Fail-fast preserves invariants.** Scope 2 and 3 do not run if scope 1 fails; the operator's first signal is the first failure, not a list.
- **`monolith_bar: Unrunnable` is a SeamAudit failure, not a skip.** An unrunnable seam audit at dispatch time aborts the cluster.

### Error handling

- **`ClusterAuditFailure::PerType`** — existing FT-139 outcome.
- **`ClusterAuditFailure::Archetype { audit, stderr }`** — new.
- **`ClusterAuditFailure::ArchetypeAuditUnrunnable { audit, stderr }`** — new; distinct from Failed so operators can triage the audit harness vs the cluster output.
- **`ClusterAuditFailure::Seam { audit, family, stderr }`** — new; the load-bearing one.
- **`ClusterAuditFailure::SeamAuditUnrunnable { audit }`** — new.
- **`ClusterDispatchError::TaskTypeNotSafelyDispatchable`** — pre-dispatch refusal.
- **`ClusterDispatchError::WeakSeamCoverage`** — infrastructure-family pre-dispatch refusal.

### Boundaries

- **In scope.** The three-scope pipeline as a function; wiring into cluster_dispatch; pre-dispatch refusal + escape-hatch routing; new ClusterOutcome variants; the ClusterReport report stage (log to drive history); nine test cases.
- **Out of scope.** Authoring concrete archetype + seam audits for the first archetype — FT-160. Escape-hatch broad-worker dispatch implementation — FT-154 ships the routing; the broad worker itself is the existing FT-123 implementation. ClusterRun graph-resident artifact type — possible future slice; v1 uses drive-history logging. Parallel audit execution — sequential for v1.

## Out of scope

- First archetype's audit set — FT-160.
- Escape-hatch worker implementation — FT-154 (routing); FT-123 (broad worker itself).
- ClusterRun graph artifact — future slice.
- Parallel audit execution — sequential for v1.
- Audit retry logic — failed audits fail the cluster; retry is operator-driven via re-dispatch.
