---
id: FT-162
title: 'decision-cli: dec session list surfaces cluster-dispatch activities and renders cluster cells with parent feature/status'
phase: 4
status: complete
depends-on:
- FT-161
adrs:
- ADR-081
tests:
- TC-391
- TC-392
- TC-393
- TC-394
domains:
- api
- observability
domains-acknowledged: {}
---

## Description

Follow-on to [FT-161](FT-161). FT-161 made `dec session show` render cluster IRIs; this slice makes them **discoverable** through `dec session list` so the operator never has to know an IRI to find it. Closes the inverse of the [ADR-081](ADR-081) totality concern: every IRI surfaced by show should be reachable from list.

Today, `dec session list`'s SPARQL filters `?session a dec:Session`. That means:

- **Cluster dispatch activities** are typed `dec:ClusterDispatch` + `prov:Activity` (per FT-146's persistence path) — they never appear in `list`. An operator who hasn't memorised `urn:dec:cluster-dispatch:<task-type>/<feature>` can't reach the cost rollup show now renders.
- **Cluster cell sessions** are typed `dec:Session` so they appear, but they don't carry `dec:featureId` (it lives on the parent) and they use `dec:cellStatus` not `dec:status` — so list renders them as `feature=(no-feature) status=(pending)`, which is honest but unhelpful.

The fix is a one-query extension and a small renderer tweak. No new artifact types, no schema change.

## Functional Specification

### Inputs

- The existing list query at `crates/decision-cli/src/features/session_inspect/mod.rs::build_session_list_query`.
- `dec:Session` (cluster cell sessions from FT-146 — carry `dec:capability`, `dec:cellStatus`, `prov:wasInformedBy`).
- `dec:ClusterDispatch` (parent activities from FT-146 — carry `dec:featureId`, `dec:taskType`, `dec:clusterOutcome`, `prov:startedAtTime`).
- Slice-1 implementer-shape sessions (carry `dec:featureId`, `dec:status`, `prov:atTime`).

### Outputs

**Extended SPARQL** with a UNION across three branches:

1. **Slice-1 sessions** (unchanged): `?s a dec:Session ; OPTIONAL ?feature ; OPTIONAL ?status ; OPTIONAL ?started`.
2. **Cluster cell sessions**: `?s a dec:Session ; ?cluster prov:wasInformedBy <s> { OPTIONAL ?cluster dec:featureId ?feature } ; OPTIONAL ?s dec:cellStatus ?status ; OPTIONAL ?s prov:startedAtTime ?started`. The parent-join hops one edge via `prov:wasInformedBy` to lift the parent's `dec:featureId` onto the cell's row.
3. **Cluster dispatch activities**: `?s a dec:ClusterDispatch ; OPTIONAL ?s dec:featureId ?feature ; OPTIONAL ?s dec:clusterOutcome ?status ; OPTIONAL ?s prov:startedAtTime ?started`. Reuses the `?status` projection slot for `dec:clusterOutcome` so the row decoder stays a single shape.

Each branch projects the same four fields: `?session ?feature ?started ?status`. The renderer doesn't need to know which branch produced the row.

**Renderer no-op** for branches 1 and 2 — already correct. Branch 3 (cluster dispatch) renders naturally because the SPARQL projects `?feature` from `dec:featureId` and `?status` from `dec:clusterOutcome` directly.

**Ordering** stays `ORDER BY ?started`. Cluster IRIs that have a timestamp interleave correctly; ones without sort to the top (existing behaviour for OPTIONAL `prov:atTime`).

### State

- **Modified on-disk:** `crates/decision-cli/src/features/session_inspect/mod.rs` — the query helper + one renamed local for clarity.
- **No new files, no new artifact types, no schema change.**

### Behaviour

1. **Three-branch UNION query**. The query is built as one string but its conceptual structure is a UNION across the three patterns above. Sub-branches use `OPTIONAL` for fields that aren't required at row decode.
2. **Parent-join via `prov:wasInformedBy`**. The cluster-cell branch joins to the parent and pulls the parent's `dec:featureId` onto the cell row.
3. **clusterOutcome reuses the status column**. `audit_failed`, `succeeded`, `cell_failed`, `audit_unrunnable` are valid `?status` values for dispatch rows. The list rendering doesn't change.
4. **Stable ordering preserved**. `ORDER BY ?started` over the union; rows with no `?started` cluster at the top of the ascending sort.
5. **No double-rendering**. A cluster cell session is rendered only once even though it appears in both branch 1 (typed `dec:Session`) and branch 2 (joined through `prov:wasInformedBy`). The branches are mutually exclusive on the parent-link predicate: branch 1 matches cells whose `prov:wasInformedBy` is absent (slice-1 path), branch 2 matches cells whose `prov:wasInformedBy` is present (cluster path). Encoded as `FILTER NOT EXISTS { ?s prov:wasInformedBy ?_ }` on branch 1.

### Invariants

- **List/show totality recovers in both directions.** Every IRI surfaced by `dec session list` continues to resolve via `dec session show` (the ADR-081 invariant). Additionally, every cluster IRI that `dec session show` accepts now appears in `list`.
- **No duplicate rows for cluster cells.** Each cell IRI lands in exactly one branch via the `FILTER NOT EXISTS` guard on the slice-1 branch.
- **Empty store stays empty.** No phantom rows; same row count semantics as today.
- **Renderer untouched.** The decoder uses the same four variable names; existing test fixtures keep passing.

### Error handling

- **Cluster cell with no parent activity in the store** → falls into branch 1's slice-1 shape (no `prov:wasInformedBy`); renders with `feature=(no-feature) status=(pending)` — accurate (the cell is an orphan).
- **Cluster dispatch with no `dec:featureId`** → renders `feature=` empty; same shape as a slice-1 session without `dec:featureId`. The IRI itself still appears.
- **SPARQL failure** → bubbles through the existing `session-list SPARQL` error, no change.

### Boundaries

- **In scope.** Query extension + parent-join + 4 TCs.
- **Out of scope.** A new `dec cluster list` top-level verb (this slice keeps everything under the existing `dec session list` namespace — operators don't have to learn a new noun). JSON output mode (separate slice). Filtering by IRI prefix (`--kind cluster-dispatch`) — small UX enhancement, deferred. Per-cluster cost roll-up in the list table (operators get it via `dec session show <cluster-iri>` already). Backfill of pre-FT-146 cluster runs (still none exist).

## Out of scope

- `dec cluster list` top-level verb.
- JSON output mode.
- `--kind` filter flag.
- Per-cluster cost in the list view.
- Pre-FT-146 backfill.
