---
id: TC-451
title: escalation chain stops before the next tier when the chain budget is exhausted
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p dec-harness ft_176_chain_budget_stops_escalation
runner-timeout: 300
observes:
- graph
---

## Description

An escalation chain whose first-tier attempt consumes the declared chain budget (stub worker usage) must not dispatch the next tier: the loop driver returns the structured `budget-exceeded` failure instead of escalating. Asserts on the **graph** (the chain's attempt records show exactly one tier; the budget failure is persisted with spent-vs-declared; no tier-2 session IRI exists).
