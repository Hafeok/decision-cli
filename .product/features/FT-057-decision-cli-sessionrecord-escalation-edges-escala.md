---
id: FT-057
title: 'decision-cli: SessionRecord escalation edges (escalated_from, escalated_to, escalation_reason)'
phase: 2
status: complete
depends-on: []
adrs:
- ADR-001
- ADR-002
- ADR-004
- ADR-005
- ADR-008
- ADR-012
- ADR-013
- ADR-014
- ADR-015
- ADR-016
- ADR-017
- ADR-018
- ADR-020
- ADR-021
- ADR-022
- ADR-023
- ADR-024
- ADR-025
- ADR-027
- ADR-033
- ADR-034
- ADR-035
- ADR-036
- ADR-037
tests:
- TC-103
domains:
- data-model
- observability
domains-acknowledged: {}
---

## Description

Extend `dec:SessionRecord` with three optional fields recording escalation chain membership per [ADR-034](ADR-034) plus three input-token-breakdown fields supporting Anthropic prompt caching per PRD §8.3 and [FT-065](FT-065):

**Escalation edges:**

- `dec:escalated_from` — the prior session in the chain.
- `dec:escalated_to` — the next session in the chain.
- `dec:escalation_reason` — which trigger signal fired to cause the escalation (drawn from the [FT-055](FT-055) vocabulary).

**Token-breakdown fields (for cache-aware cost rollups):**

- `dec:input_tokens_base` — input tokens billed at the capability's `cost_input_per_m` rate.
- `dec:input_tokens_cache_write` — tokens written to the 5-minute TTL cache, billed at `cost_cache_write_5m`.
- `dec:input_tokens_cache_hit` — tokens served from cache, billed at `cost_cache_hit_per_m`.

The chain is bidirectional: `S1 → escalated_to → S2`, `S2 → escalated_from → S1`. The dispatcher escalation loop in [FT-062](FT-062) writes the escalation edges; the worker for Anthropic dispatches ([FT-064](FT-064) plus [FT-065](FT-065)) writes the token-breakdown fields by parsing the Anthropic API response metadata. This feature lands the ontology, SHACL shape, and the read API used by `dec session show`, the metrics surface, and the cache-hit-rate fitness function from [ADR-037](ADR-037).

## Functional Specification

### Inputs

- The embedded base ontology ([FT-006](FT-006)) — `dec:SessionRecord` (or `dec:Session`) class already exists, written by [FT-021](FT-021)'s dispatcher path and [FT-011](FT-011)'s implementer.
- The trigger vocabulary from [FT-055](FT-055).
- The PROV-O integration from [ADR-004](ADR-004): sessions are first-class graph entities.
- The Capability cost fields from [FT-054](FT-054) (`cost_input_per_m`, `cost_cache_hit_per_m`, `cost_cache_write_5m`, `cost_output_per_m`) used by aggregate cost rollups.

### Outputs

- New ontology terms on `dec:SessionRecord`:
  - `dec:escalated_from` (object property, optional; `sh:maxCount 1`; range `dec:SessionRecord`).
  - `dec:escalated_to` (object property, optional; `sh:maxCount 1`; range `dec:SessionRecord`).
  - `dec:escalation_reason` (xsd:string, optional; `sh:maxCount 1`; `sh:in (...trigger vocabulary...)`).
  - `dec:input_tokens_base` (xsd:integer, required; `sh:minInclusive 0`).
  - `dec:input_tokens_cache_write` (xsd:integer, required; `sh:minInclusive 0`; zero for Scaleway dispatches).
  - `dec:input_tokens_cache_hit` (xsd:integer, required; `sh:minInclusive 0`; zero for Scaleway dispatches).
  - `dec:output_tokens` already exists; constraint unchanged (`sh:minInclusive 0`).
- Extended SHACL shape `dec:SessionRecordShape`:
  - The six new properties added with their constraints.
  - A `sh:sparql` constraint enforcing bidirectional consistency: if `S1 dec:escalated_to S2`, then `S2 dec:escalated_from S1`. Symmetric requirement on the other side.
  - A `sh:sparql` constraint enforcing that `escalation_reason` is present iff `escalated_from` is present.
  - A `sh:sparql` constraint enforcing endpoint consistency: if the session's `dec:capability` resolves to a Capability with `endpoint = scaleway`, then `input_tokens_cache_write` and `input_tokens_cache_hit` must both be 0 (Scaleway has no prompt caching).
- Extended Rust type:
  ```rust
  pub struct SessionRecord {
      // … existing fields …
      pub escalated_from: Option<SessionId>,
      pub escalated_to: Option<SessionId>,
      pub escalation_reason: Option<TriggerSignal>, // reuse from FT-055
      pub input_tokens_base: u64,
      pub input_tokens_cache_write: u64,
      pub input_tokens_cache_hit: u64,
      pub output_tokens: u64,
  }
  ```
- New SPARQL helpers:
  - `core::graph::session::escalation_chain(session_id) -> Vec<SessionRecord>` — walks `dec:escalated_from` backwards to the root, then `dec:escalated_to` forwards to the leaf, returning the chain in dispatch order.
  - `core::graph::session::aggregate_chain_cost(chain) -> ChainCost { base, cache_write, cache_hit, output, total_native_currency, by_currency }` — computed at query time per [ADR-034](ADR-034)'s decision not to denormalise. The breakdown lets cost telemetry surface cache savings explicitly.
  - `core::graph::session::cache_hit_rate(session_id) -> f32` — `input_tokens_cache_hit / (input_tokens_base + input_tokens_cache_write + input_tokens_cache_hit)`; the fitness metric referenced in [ADR-037](ADR-037).

### State

- Embedded ontology + shapes bytes grow by ~50 lines.
- No backfill needed for existing sessions (the escalation fields are optional; for the token-breakdown fields, the bootstrap migration in [FT-058](FT-058) sets `input_tokens_base = <existing input_tokens>` and the two cache fields to 0 on pre-PRD Anthropic sessions).

### Behaviour

1. Extend the ontology Turtle with the six predicates.
2. Extend the SHACL shape with the three `sh:sparql` consistency constraints (bidirectional escalation, escalation_reason-iff-escalated_from, scaleway-no-cache).
3. Add the optional escalation fields and required token-breakdown fields to the Rust struct; existing deserialisers gracefully handle missing escalation values and default missing token-breakdown values to 0 (backfill case).
4. Implement `escalation_chain`, `aggregate_chain_cost`, and `cache_hit_rate` SPARQL helpers.
5. Extend `dec session show <id>` ([FT-025](FT-025)'s CLI surface) to display:
   - The escalation chain (if any), with capability id + version per session.
   - The trigger that fired for each escalation.
   - Token breakdown per session (`base / cache_write / cache_hit / output`) and aggregate cost across the chain, with currency.
   - Cache-hit rate per session (where applicable).

### Invariants

- A session's `escalated_from` either references an existing `dec:SessionRecord` in the same value stream or is absent.
- A session's `escalated_to` is absent at the time the session is written; it is set later by the dispatcher when a follow-up escalated session is dispatched. The set is a single graph write that adds the `escalated_to` triple on the prior session in the same transaction as the new session's creation.
- Bidirectional consistency holds at every write boundary (SHACL enforces this via `sh:sparql`).
- A session may have `escalated_from` without `escalated_to` (the leaf of a chain) and vice versa (the root).
- `escalation_reason` is present iff `escalated_from` is present.
- For any session whose capability has `endpoint = scaleway`: `input_tokens_cache_write = 0` and `input_tokens_cache_hit = 0`.
- For any session whose capability has `endpoint = anthropic` and a non-null `cost_cache_hit_per_m`: the worker writes `input_tokens_base`, `input_tokens_cache_write`, and `input_tokens_cache_hit` separately, parsed from `response.usage` Anthropic-specific fields.
- `cache_hit_rate` returns 0.0 for sessions with all-zero cache fields (Scaleway or any non-cacheable dispatch); undefined / NaN never returned.

### Error handling

- A graph write that sets `escalated_to` on a prior session but fails to write the new session must roll back both writes; `GraphWriter` ([FT-001](FT-001)) provides transactional semantics.
- A read on a malformed chain (orphaned `escalated_from` reference) surfaces a `SessionError::ChainBroken { session_id, missing_ref }` to the caller; `escalation_chain` does not panic.
- An unknown trigger literal in `escalation_reason` (impossible if SHACL passed; possible if the SHACL shape is out of date) returns the literal as `TriggerSignal::Unknown(String)`.
- An Anthropic dispatch that returns `usage` without the cache breakdown (older API surface, error response) → worker logs a warning and writes `input_tokens_cache_write = 0`, `input_tokens_cache_hit = 0`, attributing all input tokens to `input_tokens_base`. Cost reporting is conservative (over-estimates cost); cache-hit rate is 0.0 for that session.

### Boundaries

- **In scope.** Ontology extension, SHACL shape, Rust struct fields, SPARQL helpers, `dec session show` chain and cache display.
- **Out of scope.** Writing the escalation edges — [FT-062](FT-062). Writing the token-breakdown fields from Anthropic API responses — [FT-064](FT-064) (verifier worker refactor parses these from `response.usage`). Setting the cache breakpoint on Anthropic dispatches — [FT-065](FT-065). Choosing when to escalate — [FT-062](FT-062). Trigger signal computation — [FT-062](FT-062).

## Out of scope

- A `dec session chain <id>` standalone command (covered by extending `dec session show`).
- Visualisation of chains beyond the textual `dec session show` output (Phase 3+ if needed).
- Storing per-chain SLAs or budget caps (Phase 3+).
- 1-hour cache TTL tracking (Anthropic offers a higher-cost 1h cache write rate; not currently in the catalog or the SHACL).
