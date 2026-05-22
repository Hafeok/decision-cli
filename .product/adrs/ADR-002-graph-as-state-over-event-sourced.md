---
id: ADR-002
title: Graph-as-state over event-sourced
status: accepted
features:
- FT-058
- FT-054
supersedes: []
superseded-by: []
domains: []
scope: cross-cutting
content-hash: sha256:cc6ee3b1268b64151c4aba59ef863317c6bcad153f76e75d1c492bfae6329a6a
source-files:
- scripts/checks/graph-as-state.sh
---

## Context

Two coherent designs exist for an event-driven system over a knowledge graph:

1. **Event-sourced.** Events are the primary record; current state is a fold over the event log. Replay rebuilds state from events.
2. **Graph-as-state.** The graph is the truth; events are derived signals emitted as side-effects of mutations. Replay is a query over the graph's history.

decision-cli's substrate is Oxigraph with named-graph support and SPARQL query. Named graphs already give per-mutation history; SPARQL already gives time-travel queries via dataset selectors. Adopting event-sourcing on top would create a second source of truth (the log) that must be kept consistent with the first (the graph).

See `decision-cli-slice-1-bounds.md` §5.4.

## Decision

**Graph-as-state.** The current graph is the truth. Events are derived signals that fire as side-effects of mutations and are themselves persisted **in the graph** (in an events named graph), not in a separate log.

Concrete consequences of this stance, baked into the architecture:

- **Replay = SPARQL** over the events named graph, filtered by sequence number. There is no separate replay infrastructure.
- **Consumer offsets** are monotonic event sequence numbers, tracked by consumers themselves. The substrate does not manage offsets.
- **No event-sourced rebuild.** The current graph is recovered via named-graph history snapshots and backups, not by replaying events from zero.
- **PROV-O links** tie each event back to the mutation that caused it and forward to the artifacts it triggered (see ADR-004).

## Consequences

**Positive:**

- Single source of truth. No log-versus-state divergence to manage.
- Replay is just SPARQL — no separate replay infrastructure, no schema migration for the log format.
- Auditability is uniform: every event is queryable like every other artifact.
- Backups cover both state and event history with one operation.

**Negative / accepted costs:**

- Event retention is bounded by store growth; compaction policy must be designed as the store grows (deferred past slice 1).
- Pure event-sourced patterns (rebuilding state from zero, branching event streams for what-if analysis) are not available.
- Consumers carrying their own offsets must handle their own at-least-once / at-most-once semantics.

## Status

Accepted. Governs FT-001 (GraphWriter writes both state and events), FT-003 (events persisted in graph), FT-005 (replay = SPARQL), FT-009 (single Oxigraph store holds both).
