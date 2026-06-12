---
id: TC-440
title: session record persists per-check verdict quads queryable via session show
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_173_verdict_session_quads
runner-timeout: 300
observes:
- graph
- stdout
---

## Description

After a cluster run whose `v1` audit fails two named checks, the cluster SessionRecord mutation must persist per-check quads: check name, status, truncated detail, implicated cells, and the degradation flag. Asserts on the **graph** (SPARQL over the orchestration store finds the per-check quads under the cluster session IRI) and on **stdout** (`dec session show <cluster-iri>` renders the audit narrative — check names and statuses — from the graph alone, with the raw audit stdout absent).
