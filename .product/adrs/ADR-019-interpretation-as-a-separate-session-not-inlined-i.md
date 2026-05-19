---
id: ADR-019
title: Interpretation as a separate session, not inlined into the action session
status: proposed
features: []
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
---

## Context

[ADR-017](ADR-017) requires action-interpretation pairing. The implementation question is whether the interpretation is a *separate session* (its own `Session` artifact, its own bundle, its own model call, its own PROV-O record) or an *inline collapse* (the same worker, in the same call, emits both the action artifact and the verdict in one structured output).

`Implementing_DDD.md` §6 raises this trade explicitly: separate sessions buy a full audit trail and cleaner role boundaries; inline collapse buys lower latency and lower LLM cost, at the price of mixing the action's confidence with its own self-evaluation.

## Decision

**For Phase A, interpretation runs as a separate session, in a separate dispatch, with a separate bundle and possibly a separate model. Inline collapse is deferred.**

The shape:

- `dec:ActionSession` and `dec:InterpretationSession` are both subclasses of `dec:Session`.
- They are siblings under a `dec:DispatchGroup` ([ADR-017](ADR-017)) with explicit PROV-O edges (`prov:wasGeneratedBy`, `prov:used`).
- The interpretation session's bundle is assembled fresh — it is not the action's bundle plus a "now verify yourself" header. The verifier sees the produced artifact, the feature_spec, the bundle hash that produced the action (for audit, not for context), and the relevant TCs/ADRs. The verifier does NOT see the action session's chain-of-thought or tool calls.
- The verifier's model binding is independent. For Phase A both roles use the same hardcoded model ([ADR-008](ADR-008)-style); but the binding is per-role so the architectural shape supports asymmetric pairing (e.g. Sonnet writes, Opus verifies) as a Phase B refinement.

### Why separate

The behavioral failure mode we need to detect is the one where the action and the interpretation agree because they are the same context, looking at the same evidence, with the same priors. Inline collapse builds that failure mode into the architecture. Separate sessions make agreement an *observable*: when the implementer says "done" and the verifier (cold-context, different bundle) also says "approved," that agreement carries signal. When the verifier rejects, that rejection carries signal. Inline collapse erases both signals.

This is the entire point of measuring action-interpretation agreement ([ADR-021](ADR-021)). The metric is only meaningful if the sessions are independent.

### When to revisit (inline collapse)

The inline pattern is correct in two cases:

1. **The verifier role's behavior is well-characterized and stable.** Once we have enough data to know that "verifier rejects iff TC fails on synthetic test," the verifier becomes a deterministic check and the separate session is overhead.
2. **The action is itself an interpretation.** A pure-analysis role (e.g. a "summarize findings" role) where the artifact IS a judgment shouldn't be re-interpreted by another LLM; the role's output schema already encodes the verdict.

Neither holds in Phase A — the implementer is a generative role with high variance and the verifier is a brand-new role with no behavior data. Both conditions for inline collapse are unmet.

When the conditions are met (Phase C earliest, plausibly later), an amendment to this ADR records the role-by-role transition. The default stays "separate" until evidence justifies inlining.

### Cost implication

Every dispatch costs two LLM calls. For slice-2 scope (implementer + verifier on decision-cli's own features), this is acceptable: the slice surface is small, the calls are small (a verifier bundle for a single feature is far smaller than the implementer's), and the cost is the price of observability.

## Rejected alternatives

- **Inline collapse from the start.** Rejected — see "Why separate" above. The framework's central claim is only testable with independent sessions.
- **Separate session, same model context (chained calls).** Rejected: bypasses the cold-context property that makes agreement meaningful. If the verifier sees the action's CoT, the sessions are coupled at the inference level even if separate at the artifact level.
- **Per-role policy flag (inline vs. separate).** Rejected for Phase A: no operator should be deciding this per-dispatch. When inline becomes correct for a role, that's a deliberate transition recorded as an ADR amendment, not a runtime toggle.

## Consequences

**Positive:**
- Action-interpretation agreement ([ADR-021](ADR-021)) is observable.
- The dispatch lifecycle is cleanly two-phase; recovery from interpretation failures is straightforward (retry the interpretation, not the whole action).
- Verifier-model selection is independent — Phase B can specialize.

**Negative / accepted costs:**
- 2× LLM cost per dispatch.
- 2× session records per dispatch in the store (acceptable; storage is cheap, queries are scoped by `DispatchGroup`).

**Enforcement:**
- The orchestrator dispatch loop has separate code paths for action and interpretation (no shared context state).
- The verifier bundle is assembled from scratch, not derived from the action's bundle. Audited by the slice-2 integration TC.

## Status

Proposed. Bound to slice 2 ([FT-021](FT-021)). Revisit per-role when the conditions for inline collapse are met.
