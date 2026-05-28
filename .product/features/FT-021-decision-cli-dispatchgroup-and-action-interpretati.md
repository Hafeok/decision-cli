---
id: FT-021
title: 'decision-cli: DispatchGroup and action-interpretation pairing in the orchestrator'
phase: 2
status: planned
depends-on:
- FT-011
- FT-019
- FT-020
adrs:
- ADR-002
- ADR-005
- ADR-017
- ADR-019
tests:
- TC-027
- TC-028
- TC-033
domains: []
domains-acknowledged: {}
---

## Description

Extend the dispatch lifecycle to enforce action-interpretation pairing per [ADR-017](ADR-017). A `DispatchGroup` becomes the parent artifact for both the action session and the interpretation session, with the dispatch advancing through new lifecycle states (`awaiting-interpretation`, `interpretation-rejected`, `awaiting-amendment`, `complete`) gated by verdicts.

This is the core orchestration feature for slice 2. It depends on [FT-019](FT-019) (verifier role) and [FT-020](FT-020) (verdict schema) and is consumed by [FT-022](FT-022) (dispatch subscription).

## Functional Specification

### Inputs

- The existing implement flow ([FT-011](FT-011)): given a feature_spec id, the orchestrator produces an action session and a `CodeChange`.
- The verifier role from [FT-019](FT-019).
- The verdict schema from [FT-020](FT-020).
- The active value-stream scope from [FT-010](FT-010).

### Outputs

- New artifact type `dec:DispatchGroup` with predicates: `dec:dispatchedFor` (the feature_spec), `dec:dispatchStatus` (the lifecycle state), `prov:wasGeneratedBy` (the action session), `prov:wasInformedBy` (the interpretation session, set when minted), `dec:inStream`.
- New lifecycle states on `dec:dispatchStatus`:
  - `awaiting-action` (mint time)
  - `awaiting-interpretation` (action terminated, verifier not yet dispatched)
  - `interpretation-running`
  - `interpretation-rejected`
  - `awaiting-amendment`
  - `action-failed`
  - `interpretation-failed`
  - `complete`
- A `core::dispatch::DispatchGroup` Rust type and state-machine helpers.
- Updated `dec implement` flow: after the action session terminates with a produced artifact, the orchestrator mints (or fetches) the `DispatchGroup`, transitions it to `awaiting-interpretation`, and emits a subscription-trigger event consumed by [FT-022](FT-022).
- Updated `dec implement` flow on completion: the verdict's value drives the final transition (`approved` → `complete`, `rejected` → `interpretation-rejected`, `amendment-required` → `awaiting-amendment`).

### State

- Every successful dispatch produces both an action session and an interpretation session, both linked to a `DispatchGroup`. The action session's status semantics from slice 1 are unchanged; the new states live on the `DispatchGroup`, not on the session.
- Per-stream working dirs ([ADR-012](ADR-012)): no change.

### Behaviour

1. Extend the ontology with `dec:DispatchGroup` and its predicates. SHACL: cardinality constraints, valid-states `sh:in` for `dec:dispatchStatus`, lifecycle-transition validation via `sh:sparql` (next-state must be in the valid-next-states set).
2. Author `core::dispatch::lifecycle` with the state machine (Rust enum + `next(current, event) -> Result<DispatchStatus, _>` function). Mirrors the SHACL rules so violations are caught at the Rust API level first; SHACL is the durable backstop.
3. Refactor `dec implement` (per the new slice-level SDP, lives in `features/ft_021_dispatch_group/` or the migrated `features/ft_003_implement/` from FT-018):
   - Mint a `DispatchGroup` at command entry. `awaiting-action`.
   - Run the slice-1 action path (FT-011).
   - On action success: transition `DispatchGroup` to `awaiting-interpretation`. Emit a `dec:dispatchReady` event consumed by [FT-022](FT-022).
   - On action failure: transition to `action-failed`. Exit with the existing error path; verifier is NOT dispatched.
4. Provide a synchronous-wait surface for the slice-2 CLI: `dec implement` blocks on the verifier session terminating, then transitions the group based on the verdict and exits. The blocking surface is `core::dispatch::await_terminal(group_iri, timeout)`.
5. SHACL refuses any write that transitions `DispatchGroup → complete` without an `approved` verdict reachable via `prov:wasInformedBy`.

### Invariants

- Every `DispatchGroup` in `complete` status has exactly one `dec:VerificationVerdict` reachable via the interpretation session with `dec:verdict = approved`.
- No `DispatchGroup` is in a state outside the enumerated set.
- Action and interpretation sessions are siblings under the same `DispatchGroup`; both carry `prov:wasInformedBy` back to the group.
- `dec:inStream` is set on the `DispatchGroup` and on both sessions ([ADR-005](ADR-005) holds).

### Error handling

- Worker (action or verifier) crashes → terminal session status `failed` → group transitions to `action-failed` or `interpretation-failed`. Group does NOT auto-retry; operator (or a Phase B+ supervisor) decides.
- Verifier dispatch fails to start (no available worker, model unreachable) → group remains in `awaiting-interpretation` past the timeout; CLI surfaces a structured error and exits non-zero. Group is recoverable: `dec verify --resume <group-iri>` re-emits the dispatch event.
- Verdict SHACL violation on commit → verdict not persisted; the verifier dispatch is failed (`interpretation-failed`). Same operator-intervention path as above.
- Concurrent writes to the same `DispatchGroup` → `StreamWriter`'s existing single-writer invariant handles serialisation.

### Boundaries

- **In scope.** `DispatchGroup` schema, state machine, `dec implement` integration, blocking await.
- **Out of scope.** The verifier dispatch subscription itself (lives in [FT-022](FT-022)). The verifier worker (FT-023). The agreement metric (FT-024). CLI extensions for paired-session display (FT-025). Automated retry / amendment loops (Phase B).

## Out of scope

- Per-role timeout policies (Phase B; uses hardcoded slice-2 default).
- Automated re-dispatch on `awaiting-amendment` (Phase B).
- Multi-action dispatch groups (a single group with multiple action sessions in series) — Phase B at earliest.
