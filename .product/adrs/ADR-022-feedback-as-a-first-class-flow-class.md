---
id: ADR-022
title: Feedback as a first-class flow class
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: cross-cutting
content-hash: sha256:ec662e4e6c682bf2caa17b40631533d3b68a78af7f2b5b7cc56838ac92e2a31d
source-files:
- crates/decision-cli/src/core/feedback/mod.rs
- crates/decision-cli/src/core/ontology/feedback.ttl
---

## Context

Phase A slice 2 ([ADR-017](ADR-017)) closes the forward half of the loop: actions are interpreted, verdicts gate completion. The remaining half is what happens when something the action discovers in flight is not the action's job to fix.

Concrete examples:

- The implementer reads the feature_spec and finds it under-specifies a critical edge case. This is a **gap**.
- The implementer finds two ADRs that contradict each other on a point the action needs to resolve. This is a **contradiction**.
- The implementer determines the feature_spec is asking for something the available tools cannot produce. This is **unimplementable**.
- The implementer notices the feature_spec has crept beyond the slice's stated bounds. This is a **scope-issue**.

Without a first-class mechanism, these emergent decisions become improvisations: the implementer guesses, picks one ADR, narrows the scope unilaterally, or writes "TODO" in the produced code. None of those leave an audit trail; none of them route the issue to a role that can fix it; none of them prevent the next dispatch from hitting the same wall.

`Implementing_DDD.md` §6 names this directly: emergent decisions during action are themselves decisions. The framework needs to treat them as such.

## Decision

**Feedback is a first-class flow class in decision-cli, distinct from forward flow at the bus and orchestration layers but using the same artifact-as-interface mechanism.**

The shape:

1. **A new artifact type `dec:Feedback`** — schema details in [ADR-023](ADR-023), [ADR-024](ADR-024). Carries a class from a controlled vocabulary, a severity, a target role, evidence (citations into the originating bundle / artifact), an optional recommendation, a lifecycle state, and PROV-O links back to the emitting session and forward to the addressing artifact (when closed).
2. **Workers emit feedback via a structured SDK call**, not by writing free-form text. The implementer worker's `emit_feedback({class: "gap", target: "spec-author", evidence: "feature_spec line 42 underspecifies …"})` produces a properly-formed `dec:Feedback` artifact through the worker harness. The harness writes it via `StreamWriter` ([ADR-005](ADR-005)).
3. **Routing is a separate concern from emission.** Workers tag *what* the feedback is and *who* it's about; the orchestrator decides where it goes via a routing table ([ADR-026](ADR-026)). Workers do not know which session or role consumes the feedback.
4. **Feedback has its own lifecycle** ([ADR-024](ADR-024)), distinct from action-session lifecycle. A feedback artifact transitions through `produced → routed → received → addressed → closed` independently of the session that emitted it.
5. **Feedback can be blocking or non-blocking** ([ADR-025](ADR-025)). Blocking feedback pauses the emitting session's dispatch; non-blocking feedback flows in parallel and the dispatch proceeds.
6. **The bus surface is shared but logically partitioned.** Feedback artifacts emit events through the same `oxi-events` substrate as forward-flow artifacts; subscriptions distinguish them by artifact type, not by transport. No new wire format, no new SSE channel.

### Why "first-class flow class" and not "tagged forward artifact"

A naive alternative is "feedback is just a `CodeChange` with a `feedback: true` tag." That fails three ways:

- **Routing semantics differ.** A `CodeChange` routes to the verifier ([ADR-017](ADR-017)). A `Feedback` artifact routes to whichever role can address the issue — the spec-author for a gap, the architect for a contradiction, etc. The routing decision depends on the feedback class, not the artifact type.
- **Lifecycle differs.** A `CodeChange` is approved-or-rejected (terminal). A `Feedback` has the four-step lifecycle in [ADR-024](ADR-024).
- **Blocking semantics differ.** A `CodeChange` never blocks its emitting session — the session already finished by the time it exists. A blocking `Feedback` *pauses* the emitting session, which is a dispatch-lifecycle effect that has no analog in forward flow.

These three differences are large enough that a tagged forward artifact would need three sets of conditional logic at the orchestrator. A distinct flow class is structurally cleaner.

### Why same bus, not separate substrate

The cost of a separate event bus (a second `oxi-events`-style crate, a second SSE channel, a second outbox) is large. The benefit (semantic separation) is achievable with subscription filters over the existing bus. `oxi-events` already speaks the vocabulary of "mutations of any artifact type"; a feedback subscription is just `?artifact a dec:Feedback`. No new transport, no new replay path.

This preserves the SDP boundary ([ADR-001](ADR-001)): `oxi-events` still has no awareness of DDD-specific concepts. The "flow class" distinction lives in `decision-cli::core` and `decision-cli::features::*`, not in the substrate.

### Where this lives in the slice-level SDP

Following the convention codified in `CLAUDE.md` "Discipline within decision-cli":

- The `dec:Feedback` artifact type, its SHACL shape, its lifecycle state machine, and the routing table are **`core/` extensions**, authored as schema-shaped feature_specs *before* slice-3 runtime features depend on them.
- The slice-3 runtime features (`features/ft_NNN_feedback_subscription/`, `features/ft_NNN_emit_feedback_sdk/`, …) consume `core::feedback` like any other shared substrate.
- Cross-feature interaction (e.g. the implementer feature's worker calling `emit_feedback`) goes through the SDK exposed by `core::worker_sdk`, not through a sibling feature.

## Rejected alternatives

- **Free-form feedback as code comments / commit messages.** Rejected: not queryable, not auditable, not routable.
- **Feedback as an out-of-band Slack message / Linear ticket.** Rejected: takes feedback out of the graph, breaks PROV-O lineage, defeats Phase C measurement.
- **Tagged forward artifacts (`CodeChange { feedback: true }`).** Rejected — see "Why first-class flow class" above.
- **Separate event bus for feedback.** Rejected — see "Why same bus" above.
- **No feedback at all for Phase A; defer to Phase B.** Rejected: the emergent-decision problem is hit immediately on dispatch #2 once Phase A wires verifier rejections. Without feedback, every rejection becomes a manual session restart.

## Consequences

**Positive:**
- Emergent decisions during action become first-class graph artifacts. Audit, replay, and measurement all work.
- Cross-role routing is graph-resident; the routing table ([ADR-026](ADR-026)) is itself reviewable.
- Phase C fitness functions can compute feedback-incidence-per-feature, time-to-addressed, repeat-feedback rate — pattern signals for the meta-loop.

**Negative / accepted costs:**
- A new artifact type, a new subscription category, a new SDK surface.
- Workers gain a non-trivial new responsibility (deciding when to emit feedback vs. proceed). Slice 3 needs explicit prompt-engineering on the worker side to make this discrimination accurate.

**Enforcement:**
- SHACL on `dec:Feedback` ([ADR-023](ADR-023), [ADR-024](ADR-024)).
- Routing-table invariant TC: every `produced` feedback artifact transitions to `routed` within one orchestrator tick.
- Cross-cutting TC under [ADR-014](ADR-014): no `Feedback` writes route around `StreamWriter`.

## Status

Proposed. Foundational for slice 3.
