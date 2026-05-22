---
id: FT-057
title: 'decision-cli: SessionRecord escalation edges (escalated_from, escalated_to, escalation_reason)'
phase: 2
status: planned
depends-on: []
adrs:
- ADR-033
- ADR-034
tests:
- TC-103
domains:
- data-model
- observability
domains-acknowledged: {}
---

## Description

Extend `dec:SessionRecord` with three optional fields recording escalation chain membership per [ADR-034](ADR-034):

- `dec:escalated_from` — the prior session in the chain.
- `dec:escalated_to` — the next session in the chain.
- `dec:escalation_reason` — which trigger signal fired to cause the escalation (drawn from the [FT-055](FT-055) vocabulary).

The chain is bidirectional: `S1 → escalated_to → S2`, `S2 → escalated_from → S1`. The dispatcher escalation loop in [FT-062](FT-062) writes these edges when escalating; this feature lands the ontology and the read API used by `dec session show` and metrics surfaces.

## Functional Specification

### Inputs

- The embedded base ontology ([FT-006](FT-006)) — `dec:SessionRecord` (or `dec:Session`) class already exists, written by [FT-021](FT-021)'s dispatcher path and [FT-011](FT-011)'s implementer.
- The trigger vocabulary from [FT-055](FT-055).
- The PROV-O integration from [ADR-004](ADR-004): sessions are first-class graph entities.

### Outputs

- New ontology terms (on `dec:SessionRecord`):
  - `dec:escalated_from` (object property, optional; `sh:maxCount 1`; range `dec:SessionRecord`).
  - `dec:escalated_to` (object property, optional; `sh:maxCount 1`; range `dec:SessionRecord`).
  - `dec:escalation_reason` (xsd:string, optional; `sh:maxCount 1`; `sh:in (...trigger vocabulary...)`).
- Extended SHACL shape `dec:SessionRecordShape`:
  - The three properties added with their constraints.
  - A `sh:sparql` constraint enforcing bidirectional consistency: if `S1 dec:escalated_to S2`, then `S2 dec:escalated_from S1`. Symmetric requirement on the other side.
  - A `sh:sparql` constraint enforcing that `escalation_reason` is present iff `escalated_from` is present (the first session in a chain has neither; later sessions have both).
- Extended Rust type:
  ```rust
  pub struct SessionRecord {
      // … existing fields …
      pub escalated_from: Option<SessionId>,
      pub escalated_to: Option<SessionId>,
      pub escalation_reason: Option<TriggerSignal>, // reuse from FT-055
  }
  ```
- New SPARQL helpers:
  - `core::graph::session::escalation_chain(session_id) -> Vec<SessionRecord>` — walks `dec:escalated_from` backwards to the root, then `dec:escalated_to` forwards to the leaf, returning the chain in dispatch order.
  - `core::graph::session::aggregate_chain_cost(chain) -> ChainCost { total_input_tokens, total_output_tokens, total_eur }` — computed at query time per [ADR-034](ADR-034)'s decision not to denormalise.

### State

- Embedded ontology + shapes bytes grow by ~30 lines.
- No backfill needed for existing sessions (the three fields are optional; pre-PRD sessions remain valid without them).

### Behaviour

1. Extend the ontology Turtle with the three predicates.
2. Extend the SHACL shape with the two `sh:sparql` consistency constraints.
3. Add the optional fields to the Rust struct; existing deserialisers gracefully handle missing values.
4. Implement `escalation_chain` and `aggregate_chain_cost` SPARQL helpers.
5. Extend `dec session show <id>` ([FT-025](FT-025)'s CLI surface) to display:
   - The escalation chain (if any), with capability id + version per session.
   - The trigger that fired for each escalation.
   - Aggregate cost across the chain.

### Invariants

- A session's `escalated_from` either references an existing `dec:SessionRecord` in the same value stream or is absent.
- A session's `escalated_to` is absent at the time the session is written; it is set later by the dispatcher when a follow-up escalated session is dispatched. The set is a single graph write that adds the `escalated_to` triple on the prior session in the same transaction as the new session's creation.
- Bidirectional consistency holds at every write boundary (SHACL enforces this via `sh:sparql`).
- A session may have `escalated_from` without `escalated_to` (the leaf of a chain) and vice versa (the root).
- `escalation_reason` is present iff `escalated_from` is present.

### Error handling

- A graph write that sets `escalated_to` on a prior session but fails to write the new session must roll back both writes; `GraphWriter` ([FT-001](FT-001)) provides transactional semantics.
- A read on a malformed chain (orphaned `escalated_from` reference) surfaces a `SessionError::ChainBroken { session_id, missing_ref }` to the caller; `escalation_chain` does not panic.
- An unknown trigger literal in `escalation_reason` (impossible if SHACL passed; possible if the SHACL shape is out of date) returns the literal as `TriggerSignal::Unknown(String)`.

### Boundaries

- **In scope.** Ontology extension, SHACL shape, Rust struct fields, SPARQL helpers, `dec session show` chain display.
- **Out of scope.** Writing the edges — [FT-062](FT-062) (dispatcher escalation loop). Choosing when to escalate — [FT-062](FT-062). Trigger signal computation — [FT-062](FT-062). Aggregate cost storage as a derived field (rejected by [ADR-034](ADR-034); computed at query time).

## Out of scope

- A `dec session chain <id>` standalone command (covered by extending `dec session show`).
- Visualisation of chains beyond the textual `dec session show` output (Phase 3+ if needed).
- Storing per-chain SLAs or budget caps (Phase 3+).
