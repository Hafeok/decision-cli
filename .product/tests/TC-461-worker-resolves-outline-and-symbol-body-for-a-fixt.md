---
id: TC-461
title: worker resolves outline and symbol body for a fixture crate through the run-scoped LSP service
type: exit-criteria
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_179_outline_via_service
runner-timeout: 600
observes:
- stdout
- exit-code
---

## Description

A dispatch against a fixture crate grants the semantic read tools; the run-scoped LSP service is spawned lazily by the harness and its endpoint passed via `code_intel_url`. The worker calls `get_document_outline` and `get_symbol_body` on a fixture file. Asserts on **stdout** (the recorded tool results name the fixture's real structs/fns — symbols the textual tools could not have produced — and the symbol body matches the fixture source) and **exit-code** (the end-to-end test passes only when the service answered; a dead service fails the run, it does not silently degrade in this test's configuration).
