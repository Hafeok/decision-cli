---
id: FT-029
title: 'decision-cli: Feedback routing subscription and table'
phase: 2
status: planned
depends-on:
- FT-002
- FT-026
- FT-027
- FT-028
- FT-030
adrs:
- ADR-003
- ADR-022
- ADR-026
tests:
- TC-039
domains: []
domains-acknowledged: {}
---

## Description

The subscription that detects newly-produced feedback artifacts and routes them per the routing table defined in [ADR-026](ADR-026). Mirrors the shape of [FT-022](FT-022) (verifier dispatch subscription) — same registry, same delivery handler pattern, different SPARQL.

## Functional Specification

### Inputs

- The subscription registry from [FT-002](FT-002).
- The `Feedback` schema from [FT-026](FT-026).
- The lifecycle state machine from [FT-027](FT-027) (this feature drives the `produced → routed` transition).
- The class vocabulary from [FT-028](FT-028) (used to look up default target roles).
- The role catalog from [FT-019](FT-019) and (extended) from [FT-030](FT-030).

### Outputs

- A seed subscription artifact installed at `dec init` and via migration:
  ```sparql
  PREFIX dec: <https://decision-cli.dev/ns#>
  SELECT ?feedback ?class ?targetOverride ?sourceSession WHERE {
    ?feedback a dec:Feedback ;
              dec:lifecycleState "produced" ;
              dec:feedbackClass ?class ;
              dec:sourceSession ?sourceSession .
    OPTIONAL { ?feedback dec:routingOverride ?targetOverride }
  }
  ```
- Trigger types: `dec:Feedback` mutations.
- Delivery handler: `core::feedback::routing::handler`.
- The routing table (Rust constants in `core::feedback::routing::table`) per [ADR-026](ADR-026).

### State

- One new `dec:Subscription` artifact per orchestration store.
- One transition per matched feedback artifact: `produced → routed` with `dec:routedAt`, `dec:targetRole` set.
- One dispatch event emitted per routed feedback (consumed by the target role's worker; for Phase A, the human via CLI as described in ADR-026's "Phase A resolution" section).

### Behaviour

1. Author the subscription Turtle seed.
2. Author the routing table in `core::feedback::routing::table` — Rust constants mapping `FeedbackClass → (default_target_role, addressing_roles, override_allowed_by)`.
3. Author the delivery handler:
   - For each `(feedback, class, targetOverride, sourceSession)` row:
     - Resolve target role: `targetOverride` if set, else `class.default_target_role()`.
     - Validate target role exists in the catalog. If not: transition feedback to `rejected` with `dec:rejectionReason = "unknown-target-role"`.
     - Validate the override (if any) is allowed for this class per the routing-table policy. If not: same `rejected` path.
     - Transition feedback to `routed` via [FT-027](FT-027)'s `transition::apply` helper. Set `dec:routedAt = now`, `dec:targetRole = resolved`.
     - Emit a `dec:Event` of class `feedback-routed` with the feedback IRI, target role, and source session. Mark for outbox publication.
4. Extend init / migration to install the seed.
5. Per the slice-level SDP: this module is `core::feedback::routing`. Every caller imports from here.

### Invariants

- For every `Feedback` in `produced` status, exactly one routing decision is made: either a `routed` transition or a `rejected` transition.
- The routing table covers every `FeedbackClass` value (compile-time check via exhaustive match).
- No routing event is emitted for a feedback whose target role does not exist (transitions to `rejected` instead).
- The handler is idempotent — re-invocation on an already-`routed` feedback is a no-op.

### Error handling

- Unknown target role → transition feedback to `rejected` with reason `unknown-target-role`. Routing event is NOT emitted. The orchestrator logs the rejection.
- Override invalid for class → transition to `rejected` with reason `override-not-permitted`.
- SHACL violation during transition write → standard `StreamWriter` error path; the orchestrator logs and retries on next subscription evaluation.
- Outbox emission failure → standard outbox retry from [FT-003](FT-003).

### Boundaries

- **In scope.** Subscription seed, routing table, delivery handler, init wiring.
- **Out of scope.** The routing table itself as a graph artifact (Phase C). Per-target-role custom dispatch event payloads (Phase B). Receipt acknowledgment from target roles ([FT-032](FT-032) and [FT-033](FT-033) handle `routed → received` transitions).

## Out of scope

- Multi-target routing (one feedback fan-out to multiple roles) — Phase C.
- Routing-table editing via CLI — Phase C.
- Routing across value streams (single-stream Phase A).
