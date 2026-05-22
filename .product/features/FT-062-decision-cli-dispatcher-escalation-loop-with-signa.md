---
id: FT-062
title: 'decision-cli: Dispatcher escalation loop with signal collection and bundle enrichment'
phase: 2
status: planned
depends-on:
- FT-057
- FT-061
adrs:
- ADR-034
tests:
- TC-109
- TC-110
- TC-111
domains:
- api
- error-handling
- observability
domains-acknowledged: {}
---

## Description

Land the dispatcher escalation loop per [ADR-034](ADR-034): after a dispatch returns, collect signals from the result + bundle + chain context, walk the role binding's escalation steps in order, fire the first matching step, enrich the bundle with the prior attempt, and dispatch again. Chain sessions are linked bidirectionally via [FT-057](FT-057)'s `escalated_from` / `escalated_to` edges. The loop terminates on no-matching-step (success) or chain-exhausted.

This is the feature where escalation becomes real. [FT-061](FT-061) makes default-capability selection graph-driven; this feature adds the tiered escalation layer on top.

## Functional Specification

### Inputs

- `ResolvedCapability` from [FT-061](FT-061).
- `RoleBinding` from [FT-055](FT-055) with its ordered `escalation_steps` and per-step triggers.
- The worker's structured result (the `CodeChange` from the implementer worker, the `VerificationVerdict` from the verifier worker, etc.).
- The bundle's `stakes` from [FT-056](FT-056).
- The trigger signal vocabulary from [FT-055](FT-055) / [ADR-034](ADR-034).
- The session record extensions from [FT-057](FT-057).

### Outputs

- New module `core::dispatcher::escalation`:
  ```rust
  pub struct DispatchAttempt {
      pub session_id: SessionId,
      pub capability: ResolvedCapability,
      pub result: WorkerResult,           // bundle-in/artifact-out output
      pub feedback: Vec<FeedbackArtifact>, // from FT-026/FT-031 emit_feedback
      pub audit_outcome: Option<AuditOutcome>, // pass/fail when audit applied
  }
  
  pub struct SignalSet {
      pub stakes: Stakes,
      pub audit_pass: Option<bool>,
      pub feedback_classes: Vec<FeedbackClass>,
      pub feedback_critical: bool,
      pub confidence: Option<f32>,
      pub prior_attempts: u32,
      pub verdict: Option<VerdictKind>,
  }
  
  pub fn collect_signals(bundle: &Bundle, attempt: &DispatchAttempt) -> SignalSet;
  
  pub fn find_next_escalation_step(
      binding: &RoleBinding,
      current_capability: &ResolvedCapability,
      signals: &SignalSet,
  ) -> Option<EscalationStep>;
  
  pub fn enrich_bundle_with_prior_attempt(
      bundle: Bundle,
      prior: &DispatchAttempt,
  ) -> Bundle;
  ```
- Extended `core::dispatcher::dispatch_role` loop per PRD §9.2:
  ```rust
  pub fn dispatch_role(graph, role_id, bundle) -> ChainResult {
      let binding = lookup_role_binding(graph, role_id)?;
      let mut capability = resolve_default_capability(graph, role_id)?;
      let mut attempt_idx = 1;
      let mut prior_attempt: Option<DispatchAttempt> = None;
      let mut chain_head: Option<SessionId> = None;
      let mut prior_session: Option<SessionId> = None;
  
      loop {
          let attempt = run_worker(graph, role_id, &bundle, &capability)?;
          record_session(graph, &attempt, prior_session, /*reason*/ ...)?;
          if let Some(p) = prior_session { link_escalated_to(graph, p, attempt.session_id)?; }
          chain_head.get_or_insert(attempt.session_id);
  
          let signals = collect_signals(&bundle, &attempt);
          let Some(next_step) = find_next_escalation_step(&binding, &capability, &signals) else {
              return ChainResult::ok(attempt, chain_head);
          };
  
          bundle = enrich_bundle_with_prior_attempt(bundle, &attempt);
          prior_attempt = Some(attempt.clone());
          prior_session = Some(attempt.session_id);
          capability = resolve_capability_by_id(graph, &next_step.step_capability)?;
          attempt_idx += 1;
      }
  }
  ```
- Bundle enrichment uses the fixed framing from [ADR-034](ADR-034) §"Bundle enrichment on escalation"; the enriched bundle is a *new* `dec:Bundle` artifact (immutable) with its own hash, linked to the original via `dec:supersedes_bundle`.
- Session linkage writes are atomic: the prior session's `escalated_to` edge and the new session's `escalated_from` + `escalation_reason` edges are written in a single transaction per `GraphWriter`.

### Signal collection

`collect_signals` maps the attempt's result into the signal set:

- `stakes` ← `bundle.stakes` ([FT-056](FT-056)).
- `audit_pass` ← `attempt.audit_outcome.passes` if an audit ran; otherwise `None`.
- `feedback_classes` ← classes of `attempt.feedback` artifacts.
- `feedback_critical` ← `any(f.severity == Critical for f in attempt.feedback)`.
- `confidence` ← `attempt.result.verdict.confidence` if the result is a `VerificationVerdict`; otherwise `None`.
- `prior_attempts` ← `attempt_idx` (the index of the *next* attempt; 1-indexed so triggers `prior_attempts_ge_3` fire on the 3rd attempt onwards).
- `verdict` ← `attempt.result.verdict.kind` if applicable.

### Trigger evaluation

`find_next_escalation_step` walks `binding.escalation_steps` in order. For each step:

- Step matches if *any* of its triggers evaluates to true against the signal set (OR within a step, by [ADR-034](ADR-034)).
- Trigger evaluation is a fixed switch:
  - `stakes_routine` → `signals.stakes == Routine`
  - `stakes_elevated` → `signals.stakes == Elevated`
  - `stakes_foundational` → `signals.stakes == Foundational`
  - `confidence_below_0.5` → `signals.confidence.map(|c| c < 0.5).unwrap_or(false)`
  - `confidence_below_0.7` → `signals.confidence.map(|c| c < 0.7).unwrap_or(false)`
  - `confidence_below_0.9` → `signals.confidence.map(|c| c < 0.9).unwrap_or(false)`
  - `audit_pass` → `signals.audit_pass == Some(true)`
  - `audit_fail` → `signals.audit_pass == Some(false)`
  - `prior_attempts_ge_N` for N ∈ {1..5} → `signals.prior_attempts >= N`
  - `feedback_contradiction` → `signals.feedback_classes.contains(Contradiction)`
  - `feedback_unimplementable_critical` → `signals.feedback_classes.contains(Unimplementable) && signals.feedback_critical`
  - `feedback_gap` → `signals.feedback_classes.contains(Gap)`
  - `feedback_scope_issue` → `signals.feedback_classes.contains(ScopeIssue)`
- Skip steps whose `step_capability` equals the current capability (no self-escalation).
- Return the first matching step, or `None` if no step matches.

### Termination

The loop terminates when:

- No escalation step matches the current signals (success path — return the last result).
- The binding's `escalation_steps` is exhausted with every step's triggers still firing (chain-exhausted — return the last result with telemetry flag `escalation_exhausted = true`).

The dispatcher does *not* impose a fixed maximum chain length; the binding's list length is the bound. Bounded-classification roles ([ADR-037](ADR-037)) have empty escalation lists so they always terminate after one attempt.

### Invariants

- Every escalated dispatch is its own `dec:SessionRecord` per [ADR-019](ADR-019)-style independence.
- The chain is bidirectionally consistent (`escalated_from` ↔ `escalated_to`) — SHACL on [FT-057](FT-057) enforces this; the dispatcher writes both edges in one transaction.
- The enriched bundle is a new `dec:Bundle` artifact with a new hash; the original bundle is not mutated.
- `find_next_escalation_step` is pure with respect to the graph (it reads no graph state; signals + binding are inputs).
- Trigger evaluation is total over the closed vocabulary — an unknown trigger string in the binding is impossible if SHACL passed.
- The dispatcher does not retry the *same* capability after a failure (escalation is always capability-changing per [ADR-034](ADR-034)).

### Error handling

- Worker failure on an attempt: the session is recorded with `exit_reason = failed`; the dispatcher proceeds to signal collection (a failed attempt's signals include `audit_fail = true`, which can drive escalation).
- Resolving the escalated capability fails (the catalog mid-flight superseded the step's target into `eol`): `EscalationError::CapabilityResolutionFailed`; the dispatcher returns the chain so far with the error noted.
- Graph write failure during chain linkage: bubble up; the entire chain attempt is recorded as failed (the prior session was already written; the new session is rolled back).
- A binding with a malformed escalation step (impossible if SHACL passed): the dispatcher logs and skips that step.

### Boundaries

- **In scope.** Signal collection, trigger evaluation, escalation loop, bundle enrichment, chain linkage writes, telemetry.
- **Out of scope.** Capability resolution mechanics — [FT-061](FT-061).
- **Out of scope.** Session record schema for chain edges — [FT-057](FT-057).
- **Out of scope.** Reasoning_effort mapping — [FT-063](FT-063).
- **Out of scope.** Worker invocation details — [FT-060](FT-060).
- **Out of scope.** Meta-loop pattern detection on escalation rate (deferred to Phase 3+ per PRD §3).

## Out of scope

- Auto-rebinding based on escalation frequency (Phase 3+ per PRD §3).
- Free-form escalation expressions (rejected by [ADR-034](ADR-034)).
- Parallel escalation (running multiple capabilities concurrently and picking the best result) — escalation is strictly sequential.
- Skipping the default capability on a fresh dispatch when stakes is `foundational` (intentional: every chain starts with the role's default; if foundational always escalates immediately, that is the role binding's policy — not a dispatcher shortcut).
