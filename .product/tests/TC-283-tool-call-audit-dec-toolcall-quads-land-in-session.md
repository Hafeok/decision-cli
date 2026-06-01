---
id: TC-283
title: 'tool-call audit: dec:ToolCall quads land in session graph with all required predicates'
type: scenario
status: unimplemented
validates:
  features:
  - FT-125
  adrs:
  - ADR-050
phase: 4
observes:
- graph
runner: cargo-test
runner-args: tc_283_tool_call_quads_persisted_in_session_graph
runner-timeout: 30
---

## Description

The happy-path audit assertion. A successful dispatch with three tool calls produces three `dec:ToolCall` instances in the session's named graph, each carrying the full predicate set required by FT-125.

## Acceptance Criteria

Rust integration test at `crates/decision-cli/tests/session_audit.rs::tc_283_tool_call_quads_persisted_in_session_graph`.

Setup:

- Construct a `WorkerResponseJson` with `tool_call_audit` containing three entries:
  1. `name="read_file"`, `status="ok"`, `args_hash="aaaa...01"`, `started_at="2026-06-01T12:00:00Z"`, `ended_at="2026-06-01T12:00:00.100Z"`.
  2. `name="write_file"`, `status="ok"`, `args_hash="bbbb...02"`, …
  3. `name="run_tests"`, `status="ok"`, `args_hash="cccc...03"`, …
- Call `lifecycle::assemble_implement_outcome(...)` with a synthetic session IRI and this response.
- Inspect the resulting `oxigraph::Store` for the session's named graph.

Assertions via SPARQL `SELECT ?tc ?name ?status ?hash ?start ?end WHERE { GRAPH <session_iri> { <session_iri> dec:toolCall ?tc . ?tc a dec:ToolCall ; dec:toolName ?name ; dec:toolStatus ?status ; dec:toolArgsHash ?hash ; dec:toolStartedAt ?start ; dec:toolEndedAt ?end } }`:

- Returns exactly 3 rows.
- The set of `?name` values is `{"read_file", "write_file", "run_tests"}`.
- Every row's `?status` is `"ok"`.
- Every `?hash` is a 64-char lowercase hex string.
- `?start` and `?end` are valid `xsd:dateTime` literals with `?end >= ?start`.
- The `tool_call_iri` (`?tc`) values are unique across rows.

Negative assertion: the default graph contains no `dec:ToolCall` quads — audit lives in the session named graph, not in the default graph.
