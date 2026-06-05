---
id: FT-145
title: 'decision-cli: dec product status — surface project summary through the dec wrapper'
phase: 4
status: in-progress
depends-on: []
adrs: []
tests:
- TC-376
- TC-377
- TC-378
- TC-375
domains:
- api
domains-acknowledged: {}
---

## Description

Adds the `status` verb to dec's `product *` wrapper so operators can run `dec product status` and get the same project summary that upstream `product status` produces (per-phase feature counts, gate state, exit-criteria coverage, list of features by status). Currently `dec product status` returns `unknown subcommand 'status'`; this slice closes the gap.

The feature is **declared with `task_type: add-cli-subcommand`** in front-matter so [FT-139](FT-139)'s classifier branch routes it through the cell cluster registered in [FT-142](FT-142) instead of dispatching the broad code-writer. The six cells (clap_args / handler / registration_wiring / mcp_tool_shim / integration_test / help_doc_string) each consume only their declared upstream artifacts — substantially smaller per-cell bundles than the whole-feature blob that hit `ContextWindowExceededError` on FT-125. This slice is the first end-to-end demonstration of the cluster-pattern routing landing a real feature.

## Functional Specification

### Inputs

- `dec product status [--format text|json] [--phase N]`
- Working directory containing a `.product/` graph (resolved via the existing `ProductConfig::discover()` chain that other `dec product *` verbs already use).

### Outputs

- stdout: human-readable text summary (default) or JSON when `--format json` is passed. The summary mirrors upstream `product status`'s shape — per-phase counts, ready/blocked gates, exit criteria coverage, and the feature list by status.
- stderr: empty on success; a one-line error message on graph-load failure.
- Exit codes: `0` on success, `1` on graph-load / IO failure, `2` on usage error (unknown flag).

### Behaviour

1. Parse `dec product status` invocation in `features/product_cmd/mod.rs`'s top-level `match verb.as_str()`. Add `"status" => dispatch_status(rest)`.
2. `dispatch_status` parses the remaining args (`--format`, `--phase`), returning `ExitCode::from(2)` on unknown flags.
3. Load the graph via the existing `load_graph()` helper (the canonical `ProductConfig::discover()` + `parser::load_all_full()` + `KnowledgeGraph::build_full()` sequence from FT-136).
4. Call `product_core::status::build_project_summary(&graph)` to produce a `ProjectSummary`.
5. Render via `product_core::status::render_project_summary_text(&summary)` for text output, or `serde_json::to_string_pretty(&summary)` for JSON.
6. Print to stdout; return `ExitCode::SUCCESS`.

### Invariants

- Behavioural parity with upstream `product status` for the summary content. Byte-for-byte parity is NOT required (renderer choice may diverge slightly), but every numeric count and every feature ID surfaced by upstream must also surface here.
- The clap arg surface (`--format`, `--phase`) is a SUBSET of upstream's flag set. `--untested`, `--failing`, `--root` are deferred (out of scope) — adopters needing them can use upstream `product status` directly.
- No new dependencies. The wiring reuses `load_graph`, `ProductConfig`, `product_core::status`, and the existing dispatcher pattern.

### Error handling

- Graph load failure → stderr "Error loading graph: <detail>", exit 1.
- `--format` value other than `text` / `json` → stderr "dec product status: invalid --format value", exit 2.
- `--phase` value not a parseable u32 → stderr "dec product status: invalid --phase value", exit 2.
- Unknown flag → stderr "dec product status: unknown flag '<flag>'", exit 2.

## Out of scope

- `--untested` / `--failing` / `--root` flags (use upstream `product status` directly when these are needed).
- A JSON streaming format. The slice emits one batched JSON object via `to_string_pretty` — line-by-line / NDJSON is a separate concern.
- An MCP twin (`product_status` tool). The MCP surface is read-only by intent and the existing `product_*` tools cover the underlying data; an MCP twin can land later if a use case appears.
- Caching the assembled `ProjectSummary` between invocations. Every call rebuilds from disk; the graph is small enough that this is fine.

## Demonstration: cluster-pattern routing

This is the first feature with a `task_type:` declaration. After it lands:

1. `dec drive ship FT-145` reaches `FeatureShipPlanner::classify_for_task_type_value("FT-145", Some("add-cli-subcommand"))`.
2. Classifier returns `Action::DispatchCluster { feature_id: "FT-145", task_type_name: "add-cli-subcommand" }`.
3. Executor calls `cluster_dispatch::run` which looks up the TaskType, computes topo order over the 6 cells, and invokes the coherence audit script at the registry-declared path.
4. The cluster's per-cell LLM dispatch is a follow-on (FT-139's substrate ships a stub executor that runs only the audit); for FT-145 the implementation lands via `product implement` / hand-implementation, and the routing is validated independently.
