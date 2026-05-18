---
id: ADR-003
title: Subscriptions as first-class graph artifacts
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:35aadc663b4d3df3ebb85c8f95fb25bfbac52976284e47d6b7f6821c2c44d613
---

## Context

The subscription registry holds active `Subscription` records — SPARQL query + triggers + handler + mode. The registry could be:

1. **In-memory only.** Subscriptions registered by code at startup; lost on restart.
2. **A config file.** Declarative, but disconnected from the graph and not introspectable via SPARQL.
3. **First-class graph artifacts.** Subscriptions live in the same graph they observe, queryable, auditable, mutable through the same writer that handles everything else.

The DDD framing says the graph is the system state. Subscriptions are part of the state: which queries are active, who registered them, which mutations they observe, when they were created. Keeping subscriptions outside the graph would violate the framing's single-source-of-truth claim.

See `decision-cli-slice-1-bounds.md` §5.2.

## Decision

Subscriptions are **first-class artifacts persisted in the graph**, in a `subscriptions` named graph.

- A `Subscription` is identified by an IRI and described by triples: its SPARQL query, declared trigger types, delivery handler reference, mode (Inline | Async), creation time, and creator (if known).
- The registry loads subscriptions from the graph at startup.
- Adding, removing, or replacing a subscription is a mutation through the same `GraphWriter` (FT-001) that handles everything else.
- The registry maintains an in-memory mirror for hot-path evaluation; the persisted form is the source of truth.

## Consequences

**Positive:**

- Subscriptions survive restart without code changes.
- They are queryable: "which subscriptions observe artifact-type X?" is a SPARQL query.
- Their lifecycle is auditable via PROV-O like every other artifact (see ADR-004).
- The same MCP / CLI surfaces used to inspect anything else work on subscriptions for free.

**Negative / accepted costs:**

- Slightly more overhead to register a subscription (a write to the graph rather than an in-memory map insert).
- The registry must reconcile its in-memory mirror with the graph on any externally-driven write (acceptable: writes go through the writer, which owns both).

## Status

Accepted. Governs FT-002 (registry implementation) and FT-009 (subscriptions seeded as v0 bootstrap artifacts).
