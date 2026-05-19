---
id: FT-022
title: 'decision-cli: Verifier dispatch subscription'
phase: 2
status: planned
depends-on:
- FT-002
- FT-003
- FT-021
adrs:
- ADR-003
- ADR-005
- ADR-017
tests:
- TC-027
- TC-028
domains: []
domains-acknowledged:
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-022 produces no feedback artifacts.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-022's code is reorganised under that migration, not by this feature.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-022 does not cross or alter that boundary.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-022 produces no feedback artifacts.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-022 does not introduce or modify a role catalog entry.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-022 does not author or modify a fitness-function artifact.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-022 neither emits nor consumes verdicts.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-022's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-022 neither emits nor routes feedback.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-022 runs after the working directory is resolved and does not re-discover it.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-022 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-022 produces no new Session or event type and inherits lineage from the harness.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-022 produces no action/interpretation pair.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-022 has no feedback to gate.
---

## Description

The subscription that detects "an action session has terminated, the dispatch is in `awaiting-interpretation`, no verifier has been dispatched yet" and emits the dispatch event consumed by the verifier worker ([FT-023](FT-023)). Lives in `core/` per the slice-level SDP — subscriptions are platform substrate, not feature-volatile.

Builds on [FT-002](FT-002) (subscription registry) and [FT-003](FT-003) (event emission and outbox).

## Functional Specification

### Inputs

- The subscription registry from [FT-002](FT-002).
- The `DispatchGroup` lifecycle from [FT-021](FT-021).
- The verifier role catalog entry from [FT-019](FT-019).

### Outputs

- A seed subscription artifact installed at `dec init` (idempotent — existing slice-1 stores gain it via the bootstrap-subscription pattern from [FT-009](FT-009)):
  ```sparql
  PREFIX dec:  <https://decision-cli.dev/ns#>
  PREFIX prov: <http://www.w3.org/ns/prov#>
  SELECT ?group ?actionSession WHERE {
    ?group a dec:DispatchGroup ;
           dec:dispatchStatus "awaiting-interpretation" ;
           prov:wasGeneratedBy ?actionSession .
    ?actionSession dec:sessionStatus "completed" .
    FILTER NOT EXISTS { ?group prov:wasInformedBy ?interpretationSession }
  }
  ```
- Trigger types: `dec:DispatchGroup` and `dec:Session` mutations.
- Delivery handler: produces a `dec:VerifierDispatchEvent` with `dec:targetRole = "verifier"`, `dec:dispatchGroup = ?group`, `dec:bundleSeed = ?actionSession`'s produced artifact.
- Delivery mode: async (per [FT-002](FT-002)'s `SubscriptionMode::Async`) — the verifier worker is out-of-process.
- Emitted event is published through the outbox ([FT-003](FT-003)) and reaches the verifier worker via the SSE transport ([FT-004](FT-004)) or the in-process broadcast channel for the slice-2 happy path.

### State

- One new `dec:Subscription` artifact per orchestration store. Persistence semantics match [FT-002](FT-002): subscription is a graph artifact; the registry reads it at startup.
- One outbox row per verifier-dispatch event (matches [FT-003](FT-003) shape).

### Behaviour

1. Author the subscription Turtle seed (`crates/decision-cli/src/core/subscriptions/seeds/verifier_dispatch.ttl`).
2. Extend `dec init` (and the migration script if needed) to install the seed alongside [FT-009](FT-009)'s v0 subscriptions.
3. Wire the delivery handler in `core::subscriptions::verifier_dispatch`. On match, it:
   - Constructs the verifier bundle (the `CodeChange` produced by the action session, the originating feature_spec, the bundle hash that produced the action, the relevant TCs and cross-cutting ADRs — fetched via the slice-1 product subprocess wrapper).
   - Emits a `dec:Event` with `dec:eventClass = "verifier-dispatch"`, `dec:targetRole = "verifier"`, and the bundle as payload.
   - Marks the event for outbox publication ([FT-003](FT-003)).
4. The orchestrator transitions `dec:DispatchGroup` from `awaiting-interpretation` to `interpretation-running` when the corresponding interpretation session starts (i.e. when the verifier worker connects and acks the dispatch).

### Invariants

- For every `DispatchGroup` in `awaiting-interpretation`, exactly one verifier-dispatch event is produced. Idempotency: the subscription's `FILTER NOT EXISTS { ?group prov:wasInformedBy ?ses }` guarantees no double-dispatch.
- No verifier-dispatch event is produced for an action session in `failed` status.
- No verifier-dispatch event is produced for a `DispatchGroup` not in the active stream (the subscription's `FROM <stream-graph>` clause enforces [ADR-005](ADR-005) scope).

### Error handling

- Bundle assembly failure (e.g. product-cli subprocess error) → event is NOT published; the subscription's delivery handler returns an error and the orchestrator logs it. The `DispatchGroup` remains in `awaiting-interpretation`, allowing a manual retry via `dec verify --resume`.
- Outbox publication failure → standard outbox-retry behaviour from [FT-003](FT-003).
- Worker not connected (no consumer) → event sits in the outbox; the verifier worker picks it up on connect.

### Boundaries

- **In scope.** The subscription seed, the delivery handler in `core/`, init wiring.
- **Out of scope.** The verifier worker itself ([FT-023](FT-023)). The `DispatchGroup` schema and lifecycle ([FT-021](FT-021)). CLI surfaces ([FT-025](FT-025)).

## Out of scope

- Multi-verifier dispatch (multiple verifiers in parallel for the same group) — Phase C.
- Per-role retry policies (Phase B).
- Verifier dispatch deduplication across multiple dec processes (single-process Phase A assumption holds).
