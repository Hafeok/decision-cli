---
id: FT-179
title: 'decision-cli: semantic read tools (outline, references, symbol body, diagnostics) via a run-scoped LSP service'
phase: 5
status: planned
depends-on:
- FT-174
adrs:
- ADR-092
- ADR-091
- ADR-008
- ADR-070
tests:
- TC-461
- TC-462
- TC-463
- TC-464
- TC-469
domains:
- api
- workers
domains-acknowledged:
  ADR-081: No CLI enumerate/lookup verb pair is added; the tools are worker-facing, not dec subcommands.
  ADR-083: The service and tools are dispatch infrastructure, not a tech detail binding at archetype/instance/feature level.
  ADR-082: Tool plumbing below the TaskType layer; archetype contracts unaffected.
  ADR-084: No archetype ships or changes status; seam audits untouched.
  ADR-087: Worker read tools and the code-intel service emit no audits; verdict consumption is unaffected.
---

## Description

Slice one of [ADR-092](ADR-092): the worker gains six semantic read tools — `get_document_outline`, `get_symbol_body`, `find_definition`, `find_references`, `search_symbols`, `get_diagnostics` — served by a run-scoped, harness-owned LSP service (rust-analyzer) over a thin local HTTP endpoint, with an on-disk symbol cache keyed by content hash. This is the *pull* complement to SPMC ([ADR-091](ADR-091)/[FT-177](FT-177)): bundles stay minimal because the worker can retrieve exactly the symbol body or reference set its step needs, with the compiler as the source of truth. Adapted from the pipeline factory's code-reader server and its two-tier symbol cache.

## Functional Specification

### Inputs

- The dispatch's workspace (real checkout or [FT-115](FT-115) worktree) as the LSP root.
- Role catalog seeds: the six read tools as `dec:roleTool` entries on implementer and verifier ([ADR-070](ADR-070)); narrowable per cell ([ADR-088](ADR-088)).
- A `dec:LanguageServer` capability catalog (ADR-092 §8, amendment), seeded with one entry: rust-analyzer — language `rust`, glob `**/*.rs`, minimum version, and per-tool declarations (`outline/symbol-body/definition/references/search: full`; `diagnostics: partial` native, `full` via flycheck).
- `DispatchPayload` extended with optional `code_intel_url`.

### Outputs

- A code-intel service module in the harness: lazy rust-analyzer spawn on first semantic call, local HTTP endpoint for the run's duration, unconditional teardown with the run. The service runs an explicit `indexing → ready` state machine keyed on rust-analyzer's quiescence signal (`experimental/serverStatus`); calls during warm-up return a structured *index-warming* result while cache-served answers remain available.
- Capability resolution: the dispatch's semantic surface is **role ∩ cell ∩ language capability** for the targeted files; at spawn the catalog's declarations are verified against the `initialize` response's `ServerCapabilities`, and unadvertised tools are dropped per-tool with the mismatch recorded (ADR-092 §9).
- `get_diagnostics` in two modes: `fast` (native pull diagnostics, `textDocument/diagnostic`) and `full` (flycheck / `cargo check --message-format=json`), the latter sharing invocation logic with the [FT-172](FT-172) compile probe.
- An on-disk symbol cache keyed `(relative_path, content_hash)`, surviving across runs; per-file invalidation on content change.
- Six thin-client tools in the worker registry (`agent/tools.py`), enforcing the granted-surface rules unchanged.
- Structured symbol-not-found errors listing the symbols present in the file.
- Worker telemetry records semantic tool usage and, when `code_intel_url` is absent, the degradation to textual tools.

### State

- The cache directory under the stream working directory; no graph-resident state beyond the catalog seeds.
- The service holds a workspace index only — no dispatch state; the worker remains stateless per [ADR-008](ADR-008).

### Behaviour

1. Harness starts the service lazily; `code_intel_url` is threaded into every payload of the run once live.
2. Worker semantic tools call the endpoint; results are bounded (symbol lists and bodies, never whole-workspace dumps).
3. `get_diagnostics` returns current compiler diagnostics for a file after `didChange` notification — usable immediately after a `write_file` in the same session, without `run_build`.
4. Absent `code_intel_url` with semantic tools granted: tools are dropped from the exposed registry, dispatch proceeds textually, telemetry records the omission (fail-open per ADR-092 §7).

### Invariants

- No semantic write tool ships in this slice; `write_file` remains the only mutation path.
- The service never mutates the workspace; all six tools are read-only against the index.
- Cache correctness: a stale entry for changed content is never served (content-hash keying makes this structural).
- The worker never sees a semantic tool that cannot work for the dispatch's files: no catalog entry for the language → no semantic tools exposed; a declared-but-unadvertised capability → exactly that tool dropped. Support is queryable from the graph (catalog joined with the last handshake result).

### Error handling

- Service spawn failure or rust-analyzer crash mid-run → fail-open degradation (ADR-092 §7), recorded; never a hung worker tool call (client-side timeout per call).
- Symbol-not-found → structured error with the file's available symbols; unknown file → structured error naming the path.

### Boundaries

- Real workspaces and FT-115 worktrees only; cluster sandboxes are [FT-180](FT-180)'s overlay work.
- rust-analyzer only in this slice; additional language servers (pyright for workers/) land when a feature demands them.
- Workspace containment and secrets rules ([ADR-071](ADR-071)) apply to the client tools as to every tool.

## Out of scope

- Symbol-level writes, rename, formatting, code actions (FT-180).
- Cluster-cell sandbox support (FT-180).
- A resident cross-run daemon (ADR-092 rejected alternative).
- Pre-computing outlines into bundles (the push side is FT-177's distillation).
