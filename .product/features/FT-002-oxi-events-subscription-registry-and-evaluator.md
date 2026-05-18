---
id: FT-002
title: 'oxi-events: Subscription registry and evaluator'
phase: 1
status: planned
depends-on:
- FT-001
adrs:
- ADR-001
- ADR-003
- ADR-002
- ADR-004
- ADR-008
- ADR-005
- ADR-012
tests:
- TC-009
domains: []
domains-acknowledged: {}
---

## Description

The subscription registry stores active `Subscription` records per **ADR-003 (Subscriptions as first-class graph artifacts)** and provides the evaluator the `GraphWriter` (FT-001) calls on every commit. A `Subscription` carries a SPARQL query, declared trigger types, a delivery handler, and an inline-vs-async classification.

On commit, the evaluator selects subscriptions whose triggers overlap the mutation, executes their SPARQL queries against the post-commit snapshot, diffs against the prior result, and returns the delta — the input to FT-003 event emission.

See `decision-cli-slice-1-bounds.md` §5.2.

## Functional Specification

### Inputs

- Subscription records: `{ id, query, triggers: Set<TriggerType>, handler: DeliveryHandlerRef, mode: Inline | Async }`.
- Per-commit invocation from `GraphWriter`: post-commit snapshot, prior snapshot, the trigger types touched.

### Outputs

- `SubscriptionMatch` set per commit: `{ subscription_id, delta: Added | Removed | Changed bindings, mode }`.
- Registry-management acks: added, removed, replaced.

### State

- In-memory map of subscription id → `Subscription`, mirrored from the `subscriptions` named graph (the source of truth per ADR-003).
- Per-subscription cached "last result set" used as the diff baseline.

### Behaviour

1. On init, load persisted subscriptions from the graph.
2. On commit, intersect mutation triggers with each subscription's declared triggers; skip non-intersecting.
3. Run SPARQL against the post-commit snapshot.
4. Diff against the cached prior result; emit a `SubscriptionMatch` if non-empty.
5. Update the cache.
6. Return matches to the writer.

### Invariants

- A subscription's persisted form and in-memory form share content hash (ADR-003).
- Re-evaluation against an unchanged snapshot produces empty delta.
- Removed subscriptions produce no further deltas after the next commit.

### Error handling

- Malformed SPARQL at registration → `RegistryError::InvalidQuery`.
- SPARQL execution error at evaluation isolates to that subscription: `SubscriptionMatch { delta: Error(_) }`; does not block the commit.

### Boundaries

- The registry does NOT deliver events (FT-003 + FT-004).
- Trigger-type semantics are opaque (consumers map them to their own taxonomy).
- The registry does NOT manage subscriber liveness.

## Out of scope

- Subscription priority/scheduling beyond Inline | Async.
- Streaming SPARQL (continuous query) — re-evaluation on commit boundaries only.
- Cross-instance subscription replication.
