---
id: FT-161
title: 'decision-cli: dec session show renders cluster-dispatch and cluster-session IRIs with per-cell rollup'
phase: 4
status: complete
depends-on:
- FT-146
adrs:
- ADR-081
- ADR-050
tests:
- TC-387
- TC-388
- TC-389
- TC-390
domains:
- api
- observability
domains-acknowledged: {}
---

## Description

Follow-on slice to [FT-146](FT-146). FT-146 landed `dec:SessionRecord` per cluster cell + the `dec:ClusterDispatch` parent activity in the orchestration store; this slice extends `dec session show` so operators can render that data without writing SPARQL by hand. Closes FT-146's "Outputs / Reporting" deliverable that was deferred under boundaries.

Today, `dec session show urn:dec:cluster-session:<task-type>/<feature>/<cell>` and `dec session show urn:dec:cluster-dispatch:<task-type>/<feature>` both fail with `no Session with IRI <...>` — exactly the [ADR-081](ADR-081) totality failure mode. The existing show SPARQL hardcodes slice-1 implementer-shape fields (`prov:used` bundle, `dec:featureId`, `dec:inStream`); cluster cells carry FT-057's token fields + FT-146's framing predicates but not those. This slice teaches show to detect the IRI shape and route to the right renderer.

## Functional Specification

### Inputs

- `dec:SessionRecord` per cluster cell from [FT-146](FT-146) — carries `dec:capability`, four token-breakdown fields, `dec:cellStatus`, `dec:usageSource`, `prov:wasInformedBy → cluster`, `prov:startedAtTime`, `prov:endedAtTime`.
- `dec:ClusterDispatch` parent activity from FT-146 — carries `dec:clusterOutcome`, `dec:featureId`, `dec:taskType`, `prov:startedAtTime`, `prov:endedAtTime`.
- Capability cost rates already loaded by `core::graph::session::aggregate_chain_cost` (FT-057) — reusable here.
- The existing renderer at `crates/decision-cli/src/features/implement/session_show.rs`.

### Outputs

**Cluster-session IRI** (`urn:dec:cluster-session:*`): a focused per-cell report:

```
Cell session   urn:dec:cluster-session:add-cli-subcommand/FT-145/mcp_tool_shim
Cluster        urn:dec:cluster-dispatch:add-cli-subcommand/FT-145
Capability     https://decision-cli.dev/ns/capability/qwen3-coder/v1
Status         succeeded
Usage source   worker-reported
Started        2026-06-05T09:10:37.347+00:00
Ended          2026-06-05T09:10:45.140+00:00
Duration       7.79s

Tokens:
  input_tokens_base            14591
  input_tokens_cache_write         0
  input_tokens_cache_hit           0
  output_tokens                 1982
  total input                  14591
```

**Cluster-dispatch IRI** (`urn:dec:cluster-dispatch:*`): the aggregate rollup. Walks `prov:wasInformedBy` children, sums tokens per cell, computes cost via the capability cost map (when resolvable), totals at the foot:

```
Cluster        urn:dec:cluster-dispatch:add-cli-subcommand/FT-145
Feature        FT-145
Task type      add-cli-subcommand
Outcome        audit_failed
Started        2026-06-05T09:10:30.000+00:00
Ended          2026-06-05T09:11:02.123+00:00
Duration       32.12s

Cells (6):
  cell                    status        src                base   cw   ch   output     cost
  ─────────────────────── ───────────── ───────────────── ─────── ──── ──── ─────── ────────
  clap_args_module        failed        unreported         10099    0    0     536  €0.0047
  handler_module          succeeded     worker-reported     3528    0    0    1311  €0.0030
  help_doc_string         mechanical    unreported             0    0    0       0  €0.0000
  integration_test        succeeded     worker-reported     5208    0    0     781  €0.0030
  mcp_tool_shim           succeeded     worker-reported    14591    0    0    1982  €0.0082
  registration_wiring     mechanical    unreported             0    0    0       0  €0.0000
  ─────────────────────── ───────────── ───────────────── ─────── ──── ──── ─────── ────────
  TOTAL                                                    33426    0    0    4610  €0.0189
```

When a capability IRI does not resolve in the cost map (synthetic mechanical IRI, unknown capability), the per-cell cost cell renders `—` and is excluded from the total. The total line annotates `(partial — N cells unpriced)` when this happens.

**Other IRI shapes** fall through to the existing implementer-shape renderer unchanged. ADR-081's totality invariant holds: any IRI returned by `dec session list` continues to resolve.

### State

- **Modified on-disk:** `crates/decision-cli/src/features/implement/session_show.rs` (router + two new renderers) + a sibling module `crates/decision-cli/src/features/implement/session_show_cluster.rs` to keep file lengths under ADR-013's 400-line threshold.
- **No new artifact types, no schema change, no orchestration-store migration.** Pure read path over existing FT-146 quads.

### Behaviour

1. **IRI shape detection in `session_show::session_show`.** Before the existing SPARQL runs, match the IRI prefix:
   - `urn:dec:cluster-session:` → route to `render_cluster_cell_session`.
   - `urn:dec:cluster-dispatch:` → route to `render_cluster_dispatch`.
   - Otherwise → existing renderer (unchanged).
2. **Cluster-cell renderer.** Single SPARQL fetching the cell's predicates from FT-146 + FT-057 in one OPTIONAL block. Returns `(capability, status, usage_source, started_at, ended_at, base, cw, ch, output, parent_cluster)`. Formats per the §Outputs cell sample. Duration computed in Rust from the two RFC-3339 strings (fall back to `—` when either is missing).
3. **Cluster-dispatch renderer.** Two SPARQL queries:
   - Header: `(featureId, taskType, clusterOutcome, started_at, ended_at)` from the activity itself.
   - Children: `SELECT ?cell ?cap ?status ?src ?base ?cw ?ch ?output WHERE { ?cell prov:wasInformedBy <iri> ; ... }` ordered by `STR(?cell)` so the table is stable.
   - Cost map: reuse `core::graph::session::CapabilityCostRates` loaded once via the existing helper (the path FT-065's cache-hit metric reads). Per-cell cost = `(base * input_per_m + cw * cache_write + ch * cache_hit + output * output_per_m) / 1_000_000` in the capability's currency.
   - When child capabilities mix currencies, the total line renders one row per currency (e.g. `TOTAL EUR €0.0189`, `TOTAL USD $0.000`).
4. **Multi-iteration cluster IRIs land as a single rollup.** Re-dispatching the same cluster IRI accumulates quads (multiple `clusterOutcome` triples, repeated cell-token triples). The renderer takes `MAX(?value)` for token fields (matching the read pattern from `core::graph::session::apply_solution`) and the most-recent `clusterOutcome` by `?endedAtTime` order. A note `(N runs aggregated)` appends to the header when more than one `clusterOutcome` is observed.
5. **ADR-081 cli_pairing.** Both new IRI shapes are produced by paths that resolve via existing list verbs (none yet — `dec cluster list` is a future slice) but they're already used by operators. Pass the ADR-081 totality check by extending the test that walks `cli_pairing.rs` so it also asserts every `urn:dec:cluster-*` IRI emitted by `dec session list` resolves via show — if and when `cluster-session` IRIs land in the list output. For v1: explicit fixture-driven assertion in TC-388 covering both new IRI shapes.

### Invariants

- **No regression on the existing slice-1 renderer.** IRIs not matching either cluster prefix route through the unchanged path.
- **Totals add up.** Per-cell `base + cw + ch` sums to the aggregate `total input` for the cluster; per-cell `output` sums to the aggregate `output`; per-cell cost sums to the aggregate cost (within rounding).
- **Currency consistency.** The renderer never sums across currencies. A mixed-currency cluster shows one TOTAL line per currency.
- **Stable cell ordering.** Cells render in lexicographic order of their IRIs — independent of insert order. Two `dec session show` invocations on the same IRI produce byte-identical output.
- **Capability-cost graceful absence.** A missing cost rate produces `—` in the cell's cost column and the partial-total note; the renderer never errors.

### Error handling

- **Cluster IRI with no cells found** (parent activity present but `prov:wasInformedBy` walk returns empty) → render the header with `Cells (0):` and `TOTAL` row of zeros. Distinct from the "no Session" error.
- **Cluster IRI with no header found** (the activity itself is absent — the cell IRIs exist but the parent was never written) → fall back to fixture-driven "best effort" — synthesise the feature_id / task_type from the cells' shared IRI prefix; render with `Outcome unknown`.
- **Cell IRI with no quads at all** → existing "no Session with IRI" error, untouched.
- **SPARQL failure** → bubble through the existing `session_show` error chain.

### Boundaries

- **In scope.** Two renderers + IRI shape router + cost-map integration + stable ordering + multi-iteration aggregation + 4 TCs.
- **Out of scope.** A new top-level `dec cost` or `dec cluster` verb — reuse `dec session show`. A `dec cluster list` verb that enumerates cluster activities (later slice; pairs with this show under ADR-081 when it lands). Reconciliation against FT-096's LiteLLM telemetry POST stream (still FT-146 §Out of scope; `dec:usageSource = "litellm-telemetry"` reserved). JSON output mode for the cluster renderers (the `--format json` flag is a separate slice tied to ADR-081's structured-output groundwork). Per-tool-call breakdown (FT-125 territory).

## Out of scope

- `dec cost` / `dec cluster` top-level verbs.
- `dec cluster list` enumeration (separate slice).
- JSON output mode for the cluster renderers.
- LiteLLM telemetry reconciliation.
- Per-tool-call attribution (FT-125 territory).
- Live dashboards or cross-feature aggregates.
