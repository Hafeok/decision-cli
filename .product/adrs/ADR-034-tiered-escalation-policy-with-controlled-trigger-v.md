---
id: ADR-034
title: Tiered escalation policy with controlled trigger vocabulary
status: accepted
features:
- FT-055
- FT-057
- FT-062
- FT-054
supersedes: []
superseded-by: []
domains:
- api
- error-handling
- observability
scope: cross-cutting
content-hash: sha256:3e9fe2c4c9b1bdcd8b6b2a656fc07ab91ed1406ff16b3a260fe2990a54670aa9
---

## Context

[ADR-033](ADR-033) introduces `dec:RoleBinding` as the artifact that pairs a role with a default capability. The default capability handles the steady state. The interesting case is when the steady-state capability is *insufficient* — verifier comes back at confidence 0.4, implementer fails an audit three times in a row, architect is asked to weigh in on a foundational schema change. The framework needs a structural answer for "now what?" that does not collapse into ad-hoc retries with the same model.

Three design forces collide here:

1. **Escalation must be policy, not code.** A meta-loop that proposes changing the verifier's escalation threshold from 0.7 to 0.65 should be able to rewrite a single `dec:RoleBinding` artifact, not patch a Python conditional. This rules out hardcoded thresholds and bespoke retry logic.
2. **Escalation needs leverage, not just retry.** Re-running the same prompt against a stronger model and ignoring the prior attempt is wasteful. The escalated capability should see the prior attempt's output as input ("Tier-N produced this; agree, refute, or refine") — otherwise the only thing you bought is whatever the stronger model would have produced cold.
3. **Triggers must be drawn from a closed vocabulary.** A free-form expression engine ("escalate when `confidence < 0.7 and bundle.feature_class == 'auth'`") is a slow descent into a custom DSL. A controlled vocabulary of trigger signals — `confidence_below_0.7`, `audit_fail`, `feedback_unimplementable_critical`, `stakes_foundational`, `prior_attempts_ge_3` — is enough for the meta-loop to author and revise, and small enough for the dispatcher to evaluate with a switch statement.

See the parent PRD: §6 (RoleBinding schema and escalation steps), §9 (dispatcher escalation logic), §11.2 (acceptance tests for escalation chains).

## Decision

Escalation is a graph-resident, signal-driven, prior-aware mechanism with the following shape:

### Trigger signal vocabulary

Triggers are strings drawn from a controlled vocabulary. The dispatcher implements one predicate per signal; no free-form expressions are accepted. The initial vocabulary:

- **Stakes triggers** — `stakes_routine`, `stakes_elevated`, `stakes_foundational` (set by the bundle composer per [ADR-035](ADR-035)).
- **Confidence triggers** — `confidence_below_0.5`, `confidence_below_0.7`, `confidence_below_0.9` (read from the prior attempt's `VerificationVerdict.confidence`).
- **Audit triggers** — `audit_pass`, `audit_fail` (from harness-side audit checks attached to the attempt result).
- **Attempt-count triggers** — `prior_attempts_ge_1` through `prior_attempts_ge_5` (attempt index in the current escalation chain).
- **Feedback triggers** — `feedback_contradiction`, `feedback_unimplementable_critical`, `feedback_gap`, `feedback_scope_issue` (classes drawn from [ADR-023](ADR-023); the `_critical` suffix requires `severity=critical` per [ADR-024](ADR-024)).

Adding a new trigger is itself a feature_spec — it requires extending the dispatcher's switch and SHACL constraint, and the graph stores the trigger as a versioned vocabulary. This bounds the search space for meta-loop proposals; "invent a new trigger" is a process step, not a runtime escape hatch.

### Bundle enrichment on escalation

When the dispatcher escalates, the new dispatch's bundle is the original bundle *plus* a structured "prior attempt" block:

```
## Prior attempt (tier N, capability X, model Y)

<prior result body, verbatim>

---

The above was produced by tier-N reasoning. Your task is to agree, refute,
or refine. If you agree, state which evidence in the bundle supports the
prior verdict. If you refute, cite the specific evidence the prior attempt
missed. Do not restate the prior attempt without referencing it.
```

This framing is fixed (not per-role policy) and lives in `core::bundle::enrich_with_prior_attempt`. The escalated capability sees both the original bundle and the prior tier's output, with explicit framing about what to do with it. This is what distinguishes escalation from "retry until lucky".

### Session linkage

Sessions in an escalation chain are linked bidirectionally:

- `S1 → dec:escalated_to → S2`
- `S2 → dec:escalated_from → S1`
- Each escalated session records `dec:escalation_reason` naming the trigger signal that fired.

The first session in the chain has no `escalated_from`; the last has no `escalated_to`. Cost across the chain is summed at query time, not stored as a derived field. See [FT-057](FT-057).

### Dispatcher loop

The dispatcher implements one loop (specified in PRD §9.2). The loop terminates when:

- No escalation step matches the collected signals (success path — return the current result).
- The binding's escalation chain is exhausted with the current step still triggering (return the last tier's result and flag the dispatch as `escalation_exhausted` in telemetry).

The loop does not retry the *same* capability after a failure; that is the worker's concern (the verifier already re-prompts once on validation failure per [ADR-020](ADR-020)). Escalation steps are always capability-changing.

## Consequences

**Positive.**

- Escalation behavior is fully described by `dec:RoleBinding.escalation_steps` ordered lists. The meta-loop can read, propose changes to, and validate escalation policy as graph data.
- The fixed trigger vocabulary keeps the dispatcher simple — no expression parser, no per-role mini-DSL.
- Bundle enrichment turns escalation into a *deliberation chain* rather than a retry loop. The cost of escalation buys actual leverage over the prior attempt.
- The bidirectional session linkage makes the chain visible to inspection tools (`dec session show`, `dec metrics`) without requiring derived storage.

**Negative / accepted costs.**

- The trigger vocabulary is closed. A scenario that doesn't fit the vocabulary forces an ADR amendment + vocabulary extension; there is no in-band workaround. This is the intended pressure (it forces escalation policy to evolve through the same authoring path as the rest of the framework) but it is a real friction.
- Bundle enrichment increases token count on every escalated dispatch. The cost is bounded by the prior result's size (typically a `VerificationVerdict` or `CodeChange`, not the original bundle), but it is not zero.
- Sessions in long escalation chains can fan out cost in ways the operator may not expect. The dispatcher must surface chain length and aggregate cost in `dec session show` telemetry so escalations do not silently accumulate.
- The dispatcher becomes the source of truth for which signals are computable. Adding a signal (e.g. `latency_above_30s`) requires changing the dispatcher *and* the vocabulary at once — they cannot drift.

**Boundary enforcement.**

- Workers do not compute signals. The harness collects signals from the worker's `result` plus the bundle's `stakes` plus the chain's `attempt` count, in `core::dispatcher::collect_signals`.
- SHACL on `dec:EscalationTrigger` constrains `trigger_signal` to the vocabulary at write time; an unknown signal cannot enter the graph.
- The trigger vocabulary is a versioned constant in `core::dispatcher::triggers`. A vocabulary extension requires changing this constant and re-validating all existing `RoleBinding` artifacts.

## Relationship to existing ADRs

- **[ADR-022](ADR-022) (feedback as a first-class flow).** Escalation reads feedback class/severity to decide whether to escalate. Feedback is still routed independently (per [ADR-026](ADR-026)); escalation is a *response* to feedback, not its destination.
- **[ADR-024](ADR-024) (feedback lifecycle).** A `feedback_unimplementable_critical` trigger reads the feedback artifact's class and severity at signal-collection time. Lifecycle state (open/closed) is irrelevant to the escalation decision.
- **[ADR-031](ADR-031) (chain-integrity dispatch gate).** Escalation does not bypass chain integrity. Each escalated session is itself dispatched through the standard chain gate.

## Status

Proposed. Governs [FT-055](FT-055) (RoleBinding/EscalationStep/EscalationTrigger), [FT-057](FT-057) (session escalation edges), [FT-062](FT-062) (dispatcher escalation loop). Companion to [ADR-033](ADR-033) (capability routing).
