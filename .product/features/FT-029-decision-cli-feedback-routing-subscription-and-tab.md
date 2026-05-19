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
domains-acknowledged:
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-029 produces no action/interpretation pair.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-029 is out of scope for the pairing.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-029 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-029's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-029 does not introduce or modify a role catalog entry.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-029 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-029 runs after the working directory is resolved and does not re-discover it.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-029 neither emits nor consumes verdicts.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-029 produces no feedback artifacts.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-029 produces no new Session or event type and inherits lineage from the harness.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-029 does not cross or alter that boundary.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-029 has no feedback to gate.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-029 does not author or modify a fitness-function artifact.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-029 produces no feedback artifacts.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-029's code is reorganised under that migration, not by this feature.
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
