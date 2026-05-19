---
id: FT-032
title: 'decision-cli: Blocking vs non-blocking feedback in the dispatch lifecycle'
phase: 2
status: planned
depends-on:
- FT-021
- FT-026
- FT-027
- FT-031
adrs:
- ADR-017
- ADR-025
tests:
- TC-036
- TC-037
domains: []
domains-acknowledged: {}
---

## Description

Wire blocking vs non-blocking feedback into the dispatch lifecycle per [ADR-025](ADR-025). When a worker emits blocking feedback, the orchestrator pauses the `DispatchGroup` ([FT-021](FT-021)) in a new state `paused-for-feedback` and refuses to advance until all blocking feedback on the dispatch is `addressed` or `rejected`. Non-blocking feedback flows in parallel; the dispatch proceeds normally.

This extends slice 2's dispatch lifecycle and depends on every prior slice-3 feature.

## Functional Specification

### Inputs

- The `DispatchGroup` lifecycle from [FT-021](FT-021).
- The feedback lifecycle from [FT-027](FT-027).
- Worker emissions through the SDK ([FT-031](FT-031)).
- The class default-disposition mapping from [FT-028](FT-028).

### Outputs

- New `DispatchGroup` state: `paused-for-feedback`.
- Extended state machine in `core::dispatch::lifecycle`:
  - `awaiting-action → paused-for-feedback` (when the action worker emits blocking feedback)
  - `paused-for-feedback → awaiting-interpretation` (when all blocking feedback on the group is `addressed`, AND the action artifact exists OR a retry produced one)
  - `paused-for-feedback → feedback-rejected-action-blocked` (when any blocking feedback transitions to `rejected`; terminal failure mode)
- Rust API: `core::dispatch::pause_on_feedback(group_iri, blocking_feedback_iris)` and `core::dispatch::resume_check(group_iri)`.
- Subscription that watches feedback lifecycle transitions: when a feedback artifact whose `sourceSession` belongs to a paused dispatch reaches a terminal state (`addressed`, `rejected`, `closed`), the subscription calls `resume_check`.
- The slice-3 implementer flow: when [FT-031](FT-031)'s harness detects a blocking emission, it (a) drops the action artifact, (b) calls `pause_on_feedback`, (c) terminates the worker session as `paused-by-feedback` (a new session-level terminal status that does NOT trigger verifier dispatch via [FT-022](FT-022)).

### State

- One new `DispatchGroup` lifecycle state.
- One new `Session` terminal status: `paused-by-feedback`.
- One new subscription artifact.

### Behaviour

1. Extend the `DispatchGroup` state-machine table with the three new transitions.
2. Extend the SHACL constraints to recognise the new state.
3. Author `core::dispatch::pause_on_feedback`:
   - Validate the group is in `awaiting-action` (cannot pause an already-interpreting dispatch in Phase A).
   - Transition the group to `paused-for-feedback`. Record `dec:blockedBy = [feedback IRIs]`.
4. Author `core::dispatch::resume_check`:
   - List feedback IRIs in `dec:blockedBy`.
   - If any are in non-terminal states: no-op.
   - If all are `addressed` or `closed`: dispatch a retry action with the addressing artifacts added to the bundle, transition group to `awaiting-action` (for the retry). The retry replaces the original action session; the original is preserved with `paused-by-feedback`.
   - If any is `rejected`: transition group to `feedback-rejected-action-blocked`. Terminal.
5. Author the subscription:
   ```sparql
   PREFIX dec: <https://decision-cli.dev/ns#>
   SELECT ?group WHERE {
     ?group a dec:DispatchGroup ;
            dec:dispatchStatus "paused-for-feedback" ;
            dec:blockedBy ?feedback .
     ?feedback dec:lifecycleState ?state .
     FILTER(?state IN ("addressed", "rejected", "closed"))
   }
   ```
   Delivery handler: for each row, call `resume_check`.
6. Update [FT-031](FT-031)'s harness path to call `pause_on_feedback` and drop the action artifact when blocking emissions occur.
7. The Phase A scope limits automated retry to feedback whose `addressingArtifact` is a feature_spec amendment (per [ADR-025](ADR-025)). For other addressing-artifact types, the dispatch enters `paused-for-feedback`; an operator runs `dec verify --resume <group>` (extension of [FT-025](FT-025)) to manually trigger the retry. Phase A documents this and leaves the automation for Phase B.

### Invariants

- A dispatch in `paused-for-feedback` has at least one non-terminal blocking feedback.
- A dispatch never advances to `awaiting-interpretation` while any blocking feedback is non-terminal.
- Non-blocking feedback never causes a dispatch to pause.
- Concurrent dispatches against different feature_specs are unaffected by another dispatch's pause.
- A paused dispatch's `dec:blockedBy` list is complete — adding a new blocking feedback to a paused dispatch extends the list; the dispatch remains paused.

### Error handling

- Resume attempt while feedback is still non-terminal → `resume_check` is a no-op (subscriptions retry naturally).
- Addressing artifact missing required fields (e.g. feature_spec amendment doesn't link back to the feedback) → SHACL violation on transition write; resume fails; dispatch stays paused.
- Worker crashes after emitting blocking feedback but before exiting → harness still treats the emission as authoritative, drops the (non-existent) action artifact, and pauses the dispatch.
- Multiple blocking emissions per session: the dispatch is paused on ALL of them.

### Boundaries

- **In scope.** New `DispatchGroup` state, state-machine transitions, pause/resume APIs, the resume subscription, harness wiring.
- **Out of scope.** Automated retry for non-spec-amendment addressing artifacts (Phase B). The routing subscription itself ([FT-029](FT-029)). The worker SDK ([FT-031](FT-031)).

## Out of scope

- Cross-dispatch blocking (one dispatch's blocking feedback pausing other dispatches) — Phase D meta-loop at earliest.
- Cancellation of paused dispatches (Phase B at earliest; for Phase A, paused dispatches persist until resolved or manually superseded).
- Timeout on paused dispatches (Phase B).
