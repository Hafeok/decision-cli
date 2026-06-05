---
id: FT-146
title: 'decision-cli: cluster cell dispatches persist dec:SessionRecord with token-breakdown fields for cost rollups'
phase: 4
status: complete
depends-on: []
adrs:
- ADR-081
tests:
- TC-383
- TC-384
- TC-385
- TC-386
domains: []
domains-acknowledged: {}
---

## Description

Close the gap between [ADR-050](ADR-050) ("Session as a direct materialization of PROV-O") and the cluster-dispatch path from [FT-139](FT-139): cluster cell dispatches must persist a `dec:SessionRecord` per cell, carrying the token-breakdown fields already defined by [FT-057](FT-057). Today the cluster path mints synthetic session IRIs (`urn:dec:cluster-session:<task-type>/<feature>/<cell>`) and discards them — no SessionRecord lands in the orchestration store. As a result `dec session show`, `core::graph::session::aggregate_chain_cost`, and the cache-hit fitness metric from [ADR-037](ADR-037) all return nothing for cluster-dispatched work, and the operator cannot answer "how many tokens did the last FT-XXX cluster run cost?"

Two wires are missing:

1. **Worker side.** `workers/_shared/src/_shared/model_router.py` and `scaleway_client.py` already extract `tokens_in / tokens_out / cache_write / cache_hit` from `response.usage` into an internal `_TokenCounts`, but the values stop there — they are not surfaced on `WorkerResponse`. The pipeline-worker-sdk reports them out-of-band via the LiteLLM telemetry callback from [FT-096](FT-096), but that POSTs to a remote endpoint and is not reconciled into the orchestration graph. The harness has no in-band channel to read them.
2. **Harness side.** `crates/decision-cli/src/features/drive/cluster_dispatch.rs::run_cell_dispatch` calls `run_worker` and reads `WorkerRun { response, raw_stdout }`, then returns — no `GraphWriter::insert_session_record` call. The broad-worker implementer path persists SessionRecords via the existing dispatcher; the cluster path is parallel plumbing that bypasses it.

This feature wires both sides so every cell dispatch produces a graph-resident `dec:SessionRecord` with `input_tokens_base / input_tokens_cache_write / input_tokens_cache_hit / output_tokens` populated, the `dec:capability` link pointing at the capability the cell resolved against, and the cluster as a `prov:Activity` parent that groups its cells. With that in place `aggregate_chain_cost` on the cluster activity rolls up cost across all cells of a feature, and `dec session show urn:dec:cluster-session:add-cli-subcommand/FT-145/handler_module` answers the question that motivated this feature.

## Functional Specification

### Inputs

- `dec:SessionRecord` schema and token-breakdown fields from [FT-057](FT-057) — `input_tokens_base`, `input_tokens_cache_write`, `input_tokens_cache_hit`, `output_tokens` already exist on the SHACL shape and on the Rust `SessionRecord` struct at `crates/decision-cli/src/core/ontology/session_record/types.rs`.
- The cluster executor at `crates/decision-cli/src/features/drive/cluster_dispatch.rs` from [FT-139](FT-139) — owns the per-cell dispatch loop, knows the cell name, the capability resolved, the cluster id, and the feature id.
- Worker-side usage extraction already implemented in `workers/_shared/src/_shared/model_router.py:300` (Anthropic) and `workers/_shared/src/_shared/scaleway_client.py:118` (Scaleway). Both produce a `_TokenCounts` dataclass that is currently discarded after the local call returns.
- The capability ↔ endpoint mapping from [FT-054](FT-054) — needed to enforce the SHACL constraint from FT-057 that Scaleway sessions must have `cache_write = 0` and `cache_hit = 0`.
- The PROV-O integration from [ADR-004](ADR-004) — sessions are first-class graph entities; SessionRecords live in the orchestration store as quads in the `dec:orchestration` named graph.
- The `GraphWriter` mutation chokepoint from [FT-001](FT-001) — every quad write goes through it for SHACL validation.
- The LiteLLM-as-authoritative-cost principle from [ADR-064](ADR-064) — the in-band `usage` numbers the worker reports are reconciled against the LiteLLM telemetry callback's out-of-band POSTs from [FT-096](FT-096); divergence is a fitness signal, not an error.

### Outputs

**Worker SDK — surface usage in-band on `WorkerResponse`:**

- Extend the typed `WorkerResponse` schema (the JSON the worker writes to stdout for the harness to parse) with a new optional `usage` field:
  ```python
  class WorkerResponseUsage(BaseModel):
      input_tokens_base: int
      input_tokens_cache_write: int = 0
      input_tokens_cache_hit: int = 0
      output_tokens: int

  class WorkerResponse(BaseModel):
      # ... existing fields ...
      usage: Optional[WorkerResponseUsage] = None
  ```
- The worker populates `usage` from the `_TokenCounts` the shared clients already produce. For agentic loops that make multiple LiteLLM calls (e.g. `code-writer` after FT-123), the field carries the *sum* across all calls in the dispatch; the per-call detail remains in the LiteLLM telemetry callback's POST stream.
- `code-writer`, `verify-graph-author`, and every author/quality worker (FT-126, FT-127, FT-129..FT-133) populate `usage` from their respective clients. Workers that do not invoke an LLM (e.g. mechanical cells with `model_binding_capability_id == ""`) emit `usage: None`.

**Harness — persist `dec:SessionRecord` per cell:**

- New helper `core::graph::session::insert_cluster_cell_session_record(writer, params) -> Result<()>` that:
  - Materialises the synthetic cell IRI (`urn:dec:cluster-session:<task-type>/<feature>/<cell>`) as a `dec:SessionRecord` resource.
  - Links it to the cluster IRI (`urn:dec:cluster-dispatch:<task-type>/<feature>`) via `prov:wasInformedBy` and to the resolved capability via `dec:capability`.
  - Writes the four token-breakdown fields from the worker's `usage` (zero-filling cache fields when the worker reports a flat input count).
  - Writes status (`succeeded` / `failed` / `mechanical`), latency (`prov:startedAtTime` + `prov:endedAtTime`), and the cell's role (`dec:role`).
- Modify `crates/decision-cli/src/features/drive/cluster_dispatch.rs::run_cell_dispatch`: after `run_worker` returns and before returning the cell output to the caller, call `insert_cluster_cell_session_record`. If the cell is mechanical (`model_binding_capability_id.is_empty()`), persist a session record with `status: mechanical` and all four token fields = 0 — no LLM was called, but PROV-O coverage stays uniform.
- Modify `cluster_dispatch::run` (the outer driver) to insert a parent `dec:ClusterDispatch` quad (subtype of `prov:Activity`) at start with the cluster IRI, then close it at end with overall outcome. Per-cell SessionRecords reference this parent via `prov:wasInformedBy`.

**Reporting:**

- `dec session show urn:dec:cluster-session:<task-type>/<feature>/<cell>` renders the cell's token breakdown, capability binding, status, and latency.
- `dec session show urn:dec:cluster-dispatch:<task-type>/<feature>` (the cluster activity) renders the aggregate via `aggregate_chain_cost`-style rollup adapted for siblings rather than chains: sum of `base / cache_write / cache_hit / output` across all child cells, plus computed cost in the capability's native currency.
- No new top-level `dec` verb; the existing `dec session show` is the surface.

### State

- New optional field `usage` on the worker stdout JSON contract — backwards compatible; harness reads `None` for workers that have not yet been updated, and falls through to the "no usage data" branch (writes a SessionRecord with all token fields = 0 and a `dec:usage_source = "unreported"` predicate so downstream queries can distinguish "didn't report" from "really used zero tokens").
- New `dec:ClusterDispatch` rdfs:Class (subtype of `prov:Activity`) and one new optional predicate `dec:usage_source` (xsd:string, `sh:in ("worker-reported" "litellm-telemetry" "unreported")`).
- No existing-session backfill needed — cluster dispatches up to now have no recorded SessionRecord at all; the absence is a known gap, not a corruption.
- Embedded ontology + SHACL shapes bytes grow by ~30 lines (one new class declaration + one new predicate + one shape constraint).

### Behaviour

1. **Update the typed worker response.** In `workers/_shared/src/_shared/` (the SDK consumed by every Python worker), add the `WorkerResponseUsage` model and add the optional `usage` field to `WorkerResponse`. Update the pyoxigraph/JSON serialiser to emit the field when present.
2. **Plumb usage through every worker.** Each worker's agentic loop already calls into the shared LLM clients (`model_router.py` for Anthropic, `scaleway_client.py` for Scaleway). Capture the returned `_TokenCounts`, accumulate across multi-call loops, and pass the sum into the final `WorkerResponse(...)` construction. Code-writer, verify-graph-author, and the four author/quality workers all in scope.
3. **Extend the Rust harness DispatchPayloadJson result type.** `crates/decision-cli/src/core/dispatch/payload.rs` (or wherever the worker-response decoder lives) gains a matching `Option<WorkerResponseUsage>` field. `serde` does the rest.
4. **Land `insert_cluster_cell_session_record` in core.** New helper on `core::graph::session`. Takes a `GraphWriter`, the cluster id, the cell name, the capability, the resolved endpoint, the worker's reported usage (or `None`), the status, and the timing window. Builds the quads and writes them in a single transaction.
5. **Wire it into `cluster_dispatch::run_cell_dispatch`.** Around the existing `run_worker` call: record `started_at` before, `ended_at` after, then unconditionally write the SessionRecord (mechanical cells get `status: mechanical` and zero tokens). A `run_worker` failure still writes the SessionRecord (with `status: failed` and whatever partial usage the worker reported in its error body) before bubbling the error up — PROV-O coverage is not contingent on success.
6. **Wire the cluster activity in `cluster_dispatch::run`.** Open the `dec:ClusterDispatch` activity before the first cell dispatch with `prov:startedAtTime`; close it after the audit completes with `prov:endedAtTime` and `dec:outcome ("succeeded" | "audit_failed" | "cell_failed" | "audit_unrunnable")`.
7. **Extend `dec session show` rendering.** When the IRI is a `dec:ClusterDispatch`, render the per-cell rollup table (cell name, status, tokens, cost). When the IRI is a per-cell `dec:SessionRecord` with a `prov:wasInformedBy` link, surface the parent cluster id alongside the per-cell breakdown.

### Invariants

- **Every cell dispatch produces exactly one `dec:SessionRecord`.** Mechanical, succeeded, and failed cells all produce a record. The only way a cluster dispatch produces fewer SessionRecords than its cell count is if the cluster aborted before that cell was reached; in that case the missing cells have no record, and the cluster activity's `dec:outcome` reflects the early termination.
- **Token-breakdown fields obey FT-057's SHACL.** For sessions whose capability has `endpoint = scaleway`: `input_tokens_cache_write = 0` and `input_tokens_cache_hit = 0`. The worker's reported usage is zero-filled for those fields when the capability is Scaleway; the SHACL constraint is a backstop, not the primary enforcement.
- **Cluster activity is atomic with respect to cells.** Either the cluster activity is open and at least one cell SessionRecord references it via `prov:wasInformedBy`, or the cluster activity has its `prov:endedAtTime` set (i.e., the dispatch is complete). No half-closed clusters in the graph.
- **`usage_source` is honest.** Records with `worker-reported` usage came from the worker's `WorkerResponse.usage`. Records with `litellm-telemetry` usage came from the FT-096 callback's reconciliation pass (out of scope for this feature, but the predicate is forward-compatible). Records with `unreported` are sessions where neither source produced a value — the four token fields are zero and any aggregate-cost rollup that includes such a record carries a "completeness: partial" flag.
- **No double-counting under LiteLLM telemetry reconciliation.** When FT-096's telemetry callback POSTs the same call out-of-band and a future reconciliation worker writes it back into the graph, the reconciler updates the existing SessionRecord (matched by cluster id + cell name + start time window) rather than inserting a parallel one. This feature defines the SessionRecord IRI convention that reconciliation will match against.

### Error handling

- **Worker emits `WorkerResponse` without `usage`.** Treat as `unreported`; write a SessionRecord with all four token fields = 0 and `dec:usage_source = "unreported"`. Do not error.
- **Worker emits `usage` but the values are negative or non-integer.** Worker-side validation rejects the response before the harness sees it (pydantic constraint `ge=0`). If a malformed response slips through, the SessionRecord write fails SHACL at the harness boundary; the cluster cell is marked `failed` with a `ClusterCellFailed { reason: invalid_usage }` outcome.
- **`GraphWriter::commit` fails on the SessionRecord insert.** The cell dispatch outcome upgrades from whatever it was to `ClusterCellFailed { reason: session_record_persist_failed, underlying }`. This is rare — it implies the orchestration store is unhealthy — but it must not be silent.
- **Cluster activity write fails at open or close.** Bubbled as `ClusterDispatchError::ActivityPersistFailed`; aborts the cluster before cell dispatch begins (open failure) or marks the cluster's outcome as `audit_unrunnable` with the persistence error attached (close failure).
- **A cell's worker reports a tokens-in count that, when combined with other cells of the same cluster, exceeds the capability's `context_window` even though no single cell did.** Not an error — clusters legitimately span multiple windows. Logged as a metric but does not block.

### Boundaries

- **In scope.** Worker SDK schema extension, per-worker plumbing for the 6 active LLM-backed workers, harness helper to insert per-cell SessionRecords, cluster activity quads, `dec session show` rendering for both cell and cluster IRIs, the `dec:ClusterDispatch` class + `dec:usage_source` predicate, and a SHACL constraint extending FT-057's existing endpoint-consistency rule to cover the new SessionRecords.
- **Out of scope.** Backfill of pre-existing cluster runs — they have no record; we accept that gap rather than fabricate fields. Reconciliation against [FT-096](FT-096)'s LiteLLM telemetry POST stream (`dec:usage_source = "litellm-telemetry"` is reserved here; the reconciler itself is a follow-on). A new top-level CLI verb (existing `dec session show` is the surface). UI for cluster-cost dashboards (a Phase 5 follow-on once enough cluster runs exist to make a dashboard interesting). Cost-budget alerting / per-feature spend caps. Per-tool-call usage attribution within an agentic loop (that's [FT-125](FT-125)'s territory; this feature reports the loop-aggregate only).

## Out of scope

- Reconciliation of worker-reported usage against the LiteLLM telemetry callback from FT-096 (reserved as `dec:usage_source = "litellm-telemetry"`; the reconciliation worker is a follow-on feature).
- Backfill of cluster runs already executed (no SessionRecord exists for them; we accept the gap).
- Per-LLM-call breakdown inside an agentic loop (loop-aggregate only at this layer; FT-125's ToolCall audit is the per-call surface).
- A new `dec cost` or `dec spend` verb (reuse `dec session show` with the cluster IRI).
- Cost-budget enforcement, per-feature spend caps, or alerting.
- Cross-feature aggregate-cost dashboards (Phase 5 follow-on once data exists).
- 1-hour cache TTL tracking on the Anthropic side (consistent with FT-057's deferral of the same).
