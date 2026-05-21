---
id: FT-050
title: 'decision-cli: auto-dispatch verify-graph-author on feature creation'
phase: 2
status: planned
depends-on:
- FT-045
- FT-048
- FT-049
adrs:
- ADR-030
tests:
- TC-083
- TC-084
- TC-085
domains: []
domains-acknowledged:
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-050 consumes feature-create/update events through the oxi-events public surface and emits dispatch events through the same surface, never importing from decision-cli internals.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-050 writes session and ledger state through the StreamWriter chokepoint and persists no graphs of its own.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-050 opens a pending_review Session per dispatch and records triggered_by_event_id on the dispatch event for lineage.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; the subscription is registered per stream at dec init and only fires for events within its own stream's scope.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-050 has no CLI surface and uses the stream's resolved working directory established at dec init.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-050's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-050 does not author or modify a fitness-function artifact.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-050 lives in core::subscriptions alongside verifier_dispatch, under that migration.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-050 triggers the author worker which produces an action; the pairing completes at slice-3 executor.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-050 neither emits nor consumes verdicts.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-050 produces no action/interpretation pair of its own.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-050 surfaces Gap proposals through the pending_review session but does not route them via the feedback flow in slice 2.6.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-050 produces no feedback artifacts in slice 2.6.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-050 produces no feedback artifacts in slice 2.6.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-050 has no feedback to gate.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-050 does not introduce or modify a role catalog entry — it dispatches a role whose catalog entry lives elsewhere.
---

## Description

A subscription that auto-dispatches the verify-graph-author worker ([FT-048](FT-048)) when a feature artifact lands with TCs (and per-feature env policy is satisfied). Closes the "but someone forgot to author a graph" failure mode by ensuring every new feature gets a *proposal* automatically; the human (or LLM via MCP) still reviews and accepts ([ADR-030](ADR-030) §7 Level-3).

This is **not** a CLI surface. It is a subscription living in `core::subscriptions::verify_graph_author_dispatch` modeled on `core::subscriptions::verifier_dispatch` (`crates/decision-cli/src/core/subscriptions/verifier_dispatch/mod.rs`). The subscription fires on `dec:Feature` artifact create-or-update events, decides whether to dispatch, and if so emits a `VerifyGraphAuthorDispatchEvent` that the orchestrator picks up.

One subcommand → one slice — this slice covers the subscription end-to-end (event predicate, decision logic, dispatch emission). The CLI verb that *manually* runs the same flow is [FT-049](FT-049); this slice triggers it automatically.

## Functional Specification

### Inputs

- Feature artifact event from oxi-events ([ADR-001](ADR-001)) — feature created, updated, or had a TC linked.
- [FT-045](FT-045)'s coverage primitive — to decide "does this feature *need* a proposal?"
- The env catalog — to enumerate the envs to propose against.
- Per-stream config: which envs to auto-propose against (default: every env in the catalog whose `safety_class` is `isolated`). Authored under `.dec/config.toml`:
  ```toml
  [verify_graph_author]
  auto_dispatch = true
  envs = ["ephemeral-cli"]   # subset; "*" = every isolated env
  ```

### Outputs

- A `VerifyGraphAuthorDispatchEvent` per (feature, env) pair that needs a proposal — same shape pattern as `VerifierDispatchEvent`. The orchestrator picks up each event and runs the same handler chain [FT-049](FT-049) uses (matcher → worker → proposal), but ending in `pending_review` rather than persisting.
- A `dec:Session` artifact per dispatch with status `pending_review`. The proposal lives on the session as a `dec:proposalDocument` literal (JSON). Reviewers list the session, inspect the proposal via `dec session show`, and accept via `dec verify graph generate <feature> --environment <env> --from-session <session-id>` (a thin variant of [FT-049](FT-049)'s `--accept` mode that takes the proposal from the session rather than re-running the worker).

### State

- New subscription registered at `dec init` (alongside `verifier_dispatch`).
- Subscription state: an in-store ledger of `(feature, env, last_dispatch_at)` so retries / re-fires are idempotent within a TTL (default 1 hour). This prevents an event storm of repeated dispatches when a feature is edited rapidly.
- New session status: `pending_review` (extends the existing session-status vocabulary; the slice 2 dispatch lifecycle status values continue to apply for normal completion).
- New `dec:proposalDocument` literal on `dec:Session` carrying the proposal JSON.

### Behaviour

1. Subscription receives a feature-create or feature-update event.
2. Subscription checks per-stream config — if `auto_dispatch = false`, no-op.
3. Subscription queries the feature's TCs. If zero TCs → no-op (no claim to verify yet).
4. For each env in `verify_graph_author.envs`:
   - Call [FT-045](FT-045)'s `feature_coverage` against existing graphs in that env. If complete coverage → no-op for this env.
   - Else, check the ledger: if `(feature, env)` was last dispatched within the TTL → no-op (deduplication).
   - Else, emit `VerifyGraphAuthorDispatchEvent { feature, env, bundle_hash, triggered_by_event_id }`.
5. Orchestrator picks up each event and runs the same matcher → worker pipeline [FT-049](FT-049) uses, but the persistence step is replaced by "create a `Session` artifact with `status = pending_review` and `dec:proposalDocument = proposal_json`".
6. Reviewers see the pending session in `dec session list`, inspect via `dec session show`, and accept via `dec verify graph generate <feature> --environment <env> --from-session <session-id>` (which loads the proposal JSON from the session and runs the standard acceptance path).

### Invariants

- The subscription **never persists a `VerificationGraph`**. It produces proposals; humans (or LLM-via-MCP) accept. Level 3 ([ADR-030](ADR-030) §7) is preserved.
- The chain-integrity gate ([FT-047](FT-047)) is unaffected by this subscription: an auto-generated proposal that hasn't been accepted does **not** count as coverage. A reviewer must accept before `dec implement` can dispatch without `--waive-coverage`.
- Dedup TTL is configurable but defaults to 1 hour. Set it to 0 to dispatch on every event (testing only).
- Per-env dispatch is independent — coverage in one env does not stop dispatch in another.
- The subscription is **idempotent under restart**: the ledger is persisted in the store, not in worker memory. Replaying events from a sequence number does not double-dispatch.

### Error handling

- Worker invocation failure → mark the (would-be-pending) session with `status = error` and the worker exit code; do not retry automatically (slice 3+ may add exponential backoff). The reviewer triages.
- Per-stream config missing → fall back to defaults; emit a warning event.
- Env in config does not exist in the catalog → log and skip that env; do not block other envs.

### Boundaries

- **In scope.** The subscription module, the `VerifyGraphAuthorDispatchEvent` type, the dedup ledger, the new `pending_review` session status, the `dec:proposalDocument` literal, the `--from-session` extension to [FT-049](FT-049), per-stream config wiring, registration at `dec init`.
- **Out of scope.** Auto-acceptance (Level 4) — strictly out per [ADR-030](ADR-030). Multi-env composite proposals (one session per env). Listing pending proposals as a separate verb (`dec session list --status pending_review` already covers it). Retry backoff (slice 3+).

## Out of scope

- Auto-acceptance.
- Multi-env composite proposals.
- A dedicated `dec verify pending` list verb (use `dec session list`).
- Retry / backoff on worker failure.
- Reacting to TC-creation events directly (the subscription reacts to feature events; a feature gaining a TC is a feature-update event for our purposes).
