---
id: TC-463
title: role catalog seeds semantic read tools to implementer and verifier, no semantic write tools in slice one
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_179_catalog_read_tool_seeds
runner-timeout: 300
observes:
- graph
---

## Description

Slice-one grant invariant (ADR-092 §5): SPARQL over the seeded role catalog asserts on the **graph** that every semantic read tool (`get_document_outline`, `get_symbol_body`, `find_definition`, `find_references`, `search_symbols`, `get_diagnostics`) is a `dec:roleTool` of both the implementer and verifier roles, and that no semantic write tool name (`replace_symbol_body`, `insert_after_symbol`, `insert_before_symbol`, `rename_symbol`, `format_document`) appears on any role.
