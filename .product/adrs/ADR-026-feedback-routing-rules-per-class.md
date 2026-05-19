---
id: ADR-026
title: Feedback routing rules per class
status: proposed
features: []
supersedes: []
superseded-by: []
domains: []
scope: domain
source-files:
- crates/decision-cli/src/core/feedback/routing.rs
---

## Context

[ADR-022](ADR-022) establishes the bus and the flow class; [ADR-023](ADR-023) fixes the vocabulary; [ADR-024](ADR-024) defines the lifecycle. The remaining seam is *routing*: when a `Feedback` artifact transitions to `routed`, which role's dispatch queue does the event land on?

The routing rules are policy. In Phase C they become a graph artifact (versionable, reviewable, supersedable). In slice 3 they live in `core/` Rust source. This ADR documents the rules *as if* they were a graph artifact — same shape, same fields — so that Phase C's migration is a transcription, not a rewrite.

## Decision

**The routing table maps `(feedbackClass, target)` pairs to a target role. The default per-class targets are listed below. Per-emission overrides are allowed and recorded on the feedback artifact.**

### Routing table

| Feedback class | Default target | Phase A resolution |
|---|---|---|
| `gap` | `spec-author` | The human (until a `spec-author` role exists in Phase B). The feedback transitions to `received` when the human acknowledges via `dec feedback receive <id>`; transitions to `addressed` when a feature_spec / ADR amendment is linked via `dec feedback close <id> --addressing <artifact-uri>`. |
| `contradiction` | `architect` | The human (until Phase B's architect role). Same manual path as `gap`. |
| `unimplementable` | `spec-author` | Same manual path as `gap`. Often loops to `capability-request` as a downstream effect. |
| `scope-issue` | `slice-curator` | The human (until Phase B's slice-curator role). Non-blocking, so manual addressing is on a slower clock. |
| `defect` | `verifier` if the dispatch had a verifier verdict, else `self` (the emitter's role) | When `verifier`: routes to the same verifier role that interpreted the dispatch, with the defect artifact added to its next bundle for re-evaluation. When `self`: enqueued as additional context for the next dispatch of the same role. |
| `capability-request` | `architect` | Phase A: routes to the human as a planning signal; no automated addressing. Phase B+ folds into the planning loop. |

### Routing table semantics

A routing entry is a tuple `(class, default-target-role, override-allowed-by, addressing-roles)`:

- **`class`** — one of the six values from [ADR-023](ADR-023).
- **`default-target-role`** — the role identifier the feedback is dispatched to when no override is specified.
- **`override-allowed-by`** — which actors may override the default target (the emitting role, the orchestrator under policy, or a human). For Phase A: the emitting role may override its own emissions; humans may override via CLI.
- **`addressing-roles`** — which roles' artifacts count as valid addressing artifacts for this class. E.g. only feature_spec amendments or new ADRs validly address a `gap`; arbitrary `CodeChange` artifacts do not. Validated at lifecycle `received → addressed` transition.

### Override mechanism

When a worker emits feedback, it may specify `targetRole: <role-id>` to override the default. The orchestrator validates the target exists in the role catalog before transitioning to `routed`; an invalid target produces a `received → rejected` transition with reason `"unknown-target-role"`.

Humans override via `dec feedback route <id> --to <role-id>` (slice 3 CLI). The override is recorded on the artifact as `dec:routingOverride` with the actor's identity.

### Phase A resolution of "non-existent roles"

Phase A only has implementer (action) and verifier (interpretation) roles. The routing table above references `spec-author`, `architect`, `slice-curator` — roles that don't exist yet. The Phase A resolution: **all such routes terminate at the human**, surfaced via `dec feedback list --target <role-id>`. The human acts as a placeholder for the future role.

When the role lands (Phase B+), the routing table entries don't change — the role identifier was always the same. What changes is that `dec feedback list --target spec-author` starts including dispatched sessions instead of just unaddressed feedback.

This is the right shape because it makes Phase A → Phase B a *role addition*, not a routing-table migration. The routing rules persist across role-catalog evolution.

### Why a table and not a function

A function (e.g. `route(feedback) -> role`) is harder to inspect, harder to extend by amendment, and harder to migrate to a graph artifact in Phase C. A table is a graph in disguise — Phase C lifts each row into a `dec:RoutingRule` triple with no semantic change.

### Why per-emission overrides are allowed

The default is calibrated for the median case ([ADR-023](ADR-023)). Specific emissions know better. Telemetry on override frequency is a Phase C input — a row whose default is consistently overridden is a candidate for amendment.

### Routing is not authorization

A feedback being routed to role X does not mean role X must address it; it means role X is the *initial* target. If role X transitions the feedback to `rejected`, the rejection is itself routable (via a successor feedback or via human review). Phase A doesn't automate the rejection-recovery path; Phase D's meta-loop is the first place a rejected feedback might trigger further routing.

## Rejected alternatives

- **No defaults; every emission specifies its target.** Rejected: pushes routing decisions into worker prompts, increases worker variance, makes the routing table invisible at the platform layer.
- **One global "feedback inbox" the human triages.** Rejected: scales to N=1 emissions per day. Phase B will hit hundreds; the human can't be the routing layer.
- **Routing as Rust pattern-match (no table).** Rejected — see "Why a table and not a function."
- **Routing decided by the orchestrator based on graph queries (no explicit table).** Rejected: implicit routing is unreviewable. The table is the policy; SPARQL is the mechanism.

## Consequences

**Positive:**
- The routing table is the policy. It's reviewable, amendable, and migrable to a graph artifact in Phase C without semantic change.
- Phase A → Phase B role additions are routing-table-stable.
- Override telemetry recalibrates defaults.

**Negative / accepted costs:**
- Six classes × multiple targets means the table will grow when new roles land. Acceptable; the table is small (≤ ~20 entries projected for Phase B).
- Phase A targets terminate at humans for several classes. That's a deliberate trade — the routing shape is right; the addressing latency is the cost of not having those roles yet.

**Enforcement:**
- A slice-3 TC seeds one feedback of each class, runs the orchestrator's routing pass, asserts each routed to the correct default.
- A slice-3 TC asserts an override with an invalid target produces a `received → rejected` transition.
- The routing table lives in `core::feedback::routing`; changes to the table require an ADR-026 amendment (governed by `source-files` on this ADR).

## Status

Proposed. Linked to [FT-029](FT-029). Becomes a Phase C graph-artifact migration target.
