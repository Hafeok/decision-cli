---
id: FT-125
title: 'decision-cli: dec:ToolCall audit quads persisted in session graph for every tool invocation'
phase: 4
status: planned
depends-on:
- FT-123
adrs:
- ADR-070
- ADR-050
- ADR-071
tests:
- TC-283
- TC-284
domains:
- data-model
- observability
domains-acknowledged:
  data-model: "Introduces `dec:ToolCall` class and predicates `dec:toolName`, `dec:toolStatus`, `dec:toolArgsHash`, `dec:toolStartedAt`, `dec:toolEndedAt`. Quads land in the session named graph (no new top-level graphs)."
  observability: "Every tool invocation made by a worker is recorded as an auditable graph fact. Downstream SPARQL queries can analyse tool usage by role, by session, by status. Failure visibility is first-class — blocked or timed-out tools land in the audit too."
---

## Description

FT-123 captures every tool invocation in `WorkerResponse.tool_calls`. This feature persists those records as `dec:ToolCall` quads in the session's named graph, so `dec session show <id>` can render the audit trail and downstream SPARQL queries can analyse tool usage across sessions.

The pipeline factory does the same thing via a JSONL `audit_log_path` (`mcp-servers/src/code-writer/index.ts`). We do better — quads in the graph win on queryability, traceability ([ADR-050](ADR-050) provenance), and storage uniformity.

This is a small Rust-side feature: a quad-builder per tool call, called from `assemble_implement_outcome` in `lifecycle.rs` (or wherever the worker response is consumed). The Python side already produces the data in FT-123; we only need to write it down.

## Functional Specification

### Inputs

`WorkerResponse.tool_calls` from FT-123 — each entry carries `(name, args, status, started_at, ended_at)` already.

### Outputs

For every `ToolCall` on the accepted `WorkerResponse`, the harness writes the following quads into the session's named graph:

```ttl
<session_iri> dec:toolCall <tool_call_iri> .
<tool_call_iri> a dec:ToolCall ;
    dec:toolName "<name>" ;
    dec:toolStatus "<status>" ;          # "ok" | "error"
    dec:toolArgsHash "<sha256-hex>" ;    # of the JSON-serialised args
    dec:toolStartedAt "<iso8601>"^^xsd:dateTime ;
    dec:toolEndedAt   "<iso8601>"^^xsd:dateTime .
```

The `tool_call_iri` is opaque (`https://decision-cli.dev/ns/tool-call/<uuid>`). The args are hashed, not stored verbatim, to keep the graph compact and to avoid leaking model-supplied content into the graph payload — a tool call that writes `secrets.yaml` (rejected by FT-124, but still surfaces as a tool call with `status=error`) shouldn't put `secrets.yaml` content into the graph either.

New IRI constants in `crates/decision-cli/src/core/ontology/iris.rs` (or wherever the ontology constants live):

- `TOOL_CALL_CLASS_IRI = "https://decision-cli.dev/ns#ToolCall"`
- `TOOL_CALL_PREDICATE_IRI = "https://decision-cli.dev/ns#toolCall"`
- `TOOL_NAME_IRI`, `TOOL_STATUS_IRI`, `TOOL_ARGS_HASH_IRI`, `TOOL_STARTED_AT_IRI`, `TOOL_ENDED_AT_IRI`

### Behaviour

1. Extend `WorkerResponseJson` (Rust) and `WorkerResponse` (Python) with a `tool_call_audit: Vec<ToolCallAudit>` field. Each `ToolCallAudit` carries `name: String, status: String, args_hash: String, started_at: String, ended_at: String`. (The args_hash is computed Python-side to keep argument content out of the wire payload.)
2. In `features/implement/lifecycle.rs::assemble_implement_outcome` (or the equivalent accept path), iterate `response.tool_call_audit` and call a new `core::quads::tool_call_quads(...)` helper that returns `Vec<Quad>` for the session's named graph.
3. The harness writes those quads in the same transaction as the existing session quads — atomic with the rest of the session record.
4. `dec session show <id>` (in a separate, opt-in CLI follow-up, not in this FT) can render the tool-call list. This FT only delivers the data layer; the render layer is opt-in.

### Acceptance criteria

- Given a dispatch that records 3 tool calls (2 ok, 1 error), the session's named graph contains exactly 3 `dec:ToolCall` instances, all reachable from `<session_iri> dec:toolCall ?tc`.
- Each `dec:ToolCall` carries all six predicate triples (`a`, `dec:toolName`, `dec:toolStatus`, `dec:toolArgsHash`, `dec:toolStartedAt`, `dec:toolEndedAt`).
- A failed tool call (e.g. workspace-containment violation) is recorded with `dec:toolStatus "error"` — telemetry of failures is preserved (TC-284).
- The args hash is a 64-char lowercase hex string. Re-running the same dispatch deterministically produces the same hashes (unit-testable via Python).
- Adding the audit quads does NOT mutate any other data on the session — the existing session graph contents are unchanged byte-for-byte except for the new `dec:toolCall` quads.

### Non-goals

- The `dec session show` rendering of tool-call audit. Out of scope; tracked as a follow-up.
- An `events tail` / SSE stream of tool calls as they happen. Out of scope — audit is on accept, after the dispatch terminates.
- Args storage in the graph. Arguments are hashed, not stored. If we later want full args for replay, a `dec:toolArgsRef` blob reference is a future feature_spec — not this one.
- Cross-session aggregation queries ("which tool is most expensive on average"). Once the data is in the graph, anyone can write a SPARQL query; this FT delivers the data, not the queries.

## Exit Criteria (Test Coverage)

Per [ADR-013](ADR-013), behaviours above are asserted by TCs linked to this feature.
