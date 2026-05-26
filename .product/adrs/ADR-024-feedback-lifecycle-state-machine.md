---
id: ADR-024
title: Feedback lifecycle state machine
status: accepted
features:
- FT-027
supersedes: []
superseded-by: []
domains:
- data-model
scope: domain
content-hash: sha256:ecf3fb8a151dfa11339c2d899511284c86bdd1ba90325dfa231130d5aa99904b
source-files:
- crates/decision-cli/src/core/feedback/lifecycle.rs
- scripts/checks/feedback-resume-on-addressed.sh
---

## Context

[ADR-022](ADR-022) establishes feedback as a flow class. [ADR-023](ADR-023) fixes its vocabulary. The remaining question is **the lifecycle of a single feedback artifact** — what states it can be in, what transitions are valid, what is terminal.

A naive "open vs. closed" boolean is insufficient because the feedback's journey involves multiple distinct roles:

1. The *emitter* (an action session) produces it.
2. The *orchestrator* routes it based on the routing table.
3. The *target role* receives it (potentially via a fresh dispatch).
4. The target role addresses it (produces an artifact that resolves the issue).
5. The original emitter (or a successor) closes the loop, confirming the addressing artifact resolves the feedback.

Each transition is observably distinct; each is a measurement point for Phase C; each can fail in role-specific ways. The lifecycle state machine encodes all of this.

## Decision

**`dec:Feedback` artifacts move through a five-state lifecycle plus two terminal states. Transitions are only-forward; invalid transitions are rejected at write time by SHACL+SPARQL constraints.**

### States

| State | Meaning | Owner |
|---|---|---|
| `produced` | Just emitted by an action session. Not yet routed. | emitter |
| `routed` | Orchestrator has matched the feedback to a target role per the routing table; a dispatch event for the target role has been enqueued. | orchestrator |
| `received` | The target role has begun work on the feedback (a session linked to this feedback has started). | target role |
| `addressed` | The target role has produced an artifact (an amendment, a new ADR, a new spec, …) intended to resolve the feedback. `dec:addressingArtifact` is set. | target role |
| `closed` | The emitter (or its successor session, or an automated check) has confirmed the addressing artifact does in fact resolve the feedback. Terminal. | emitter / closer |
| `rejected` | The target role determined the feedback is invalid (false positive, misclassified, duplicate). Terminal with rationale. | target role |
| `superseded` | A later feedback artifact subsumes this one; this artifact is closed by reference, not by addressing. Terminal. | orchestrator |

### Transition diagram

```
                       ┌──────────────────────────┐
                       │                          ▼
produced ──► routed ──► received ──► addressed ──► closed
   │           │           │             │
   │           │           │             └──► rejected (terminal)
   │           │           └──────────────────► rejected (terminal)
   │           └──────────────────────────────► superseded (terminal)
   └──────────────────────────────────────────► superseded (terminal)
```

### Valid transitions (enumerated)

| From → To | Trigger | Required field on the new state |
|---|---|---|
| `produced → routed` | Orchestrator matches the target role. | `dec:routedAt`, `dec:targetRole` |
| `produced → superseded` | A newer feedback emission supersedes this one before routing. | `dec:supersededBy` |
| `routed → received` | The target role's session starts. | `dec:receivingSession` |
| `routed → superseded` | A newer feedback supersedes this one. | `dec:supersededBy` |
| `received → addressed` | The target role produces an artifact intended to resolve. | `dec:addressingArtifact` |
| `received → rejected` | The target role determines the feedback is invalid. | `dec:rejectionReason` |
| `addressed → closed` | A closure check confirms the addressing artifact resolves. | `dec:closedBy` (the session or human that closed) |
| `addressed → rejected` | The closure check found the addressing artifact does not resolve; the closure attempt itself is a rejection. | `dec:rejectionReason` |

Every other transition is invalid and refused at write time.

### Terminal states

`closed`, `rejected`, `superseded` are terminal. No outgoing transitions. A new feedback artifact must be emitted if the underlying issue recurs (which is itself a Phase C signal — repeat feedback is a pattern).

### SHACL fragment (transition validation)

```turtle
@prefix dec: <https://decision-cli.dev/ns#> .
@prefix sh:  <http://www.w3.org/ns/shacl#> .

dec:FeedbackLifecycleShape a sh:NodeShape ;
    sh:targetClass dec:Feedback ;
    sh:property [
        sh:path dec:lifecycleState ;
        sh:in ( "produced" "routed" "received"
                "addressed" "closed" "rejected" "superseded" ) ;
        sh:minCount 1 ; sh:maxCount 1 ;
    ] ;
    # Each state requires its companion field.
    sh:sparql [
        sh:message "routed state requires dec:routedAt and dec:targetRole" ;
        sh:select """
            PREFIX dec: <https://decision-cli.dev/ns#>
            SELECT $this WHERE {
              $this dec:lifecycleState "routed" .
              FILTER NOT EXISTS { $this dec:routedAt ?t ; dec:targetRole ?r }
            }
        """ ;
    ] ;
    sh:sparql [
        sh:message "addressed state requires dec:addressingArtifact" ;
        sh:select """
            PREFIX dec: <https://decision-cli.dev/ns#>
            SELECT $this WHERE {
              $this dec:lifecycleState "addressed" .
              FILTER NOT EXISTS { $this dec:addressingArtifact ?a }
            }
        """ ;
    ] ;
    sh:sparql [
        sh:message "closed state requires dec:closedBy and a preceding addressed state in audit history" ;
        sh:select """
            PREFIX dec:  <https://decision-cli.dev/ns#>
            SELECT $this WHERE {
              $this dec:lifecycleState "closed" .
              FILTER NOT EXISTS {
                $this dec:closedBy ?c ;
                      dec:addressingArtifact ?a
              }
            }
        """ ;
    ] ;
    sh:sparql [
        sh:message "rejected state requires dec:rejectionReason" ;
        sh:select """
            PREFIX dec: <https://decision-cli.dev/ns#>
            SELECT $this WHERE {
              $this dec:lifecycleState "rejected" .
              FILTER NOT EXISTS { $this dec:rejectionReason ?r }
            }
        """ ;
    ] .
```

Transition validity (no `addressed → produced`, etc.) is enforced by the `StreamWriter` chokepoint: it queries the prior state in the store and refuses mutations whose new state is not in the valid-next-states set for the prior state. The set lives in `core::feedback::lifecycle`.

### Why only-forward

Reverse transitions (e.g. `addressed → received` because the addressing artifact was deleted) are tempting but break the audit trail. The correct shape is: the prior `addressed` state is preserved in history, and a *new* feedback artifact is emitted to surface the regression. The lifecycle state machine refuses to silently rewind.

Exception: amendments to a feedback artifact's *non-state* fields (rationale clarifications, additional evidence) are allowed via standard ADR-032-style amendment. The state field itself is monotonic.

### Why terminal states distinguish `closed` / `rejected` / `superseded`

Three different end conditions, three different downstream signals:

- `closed` is success — the issue surfaced, was addressed, was verified.
- `rejected` is "the system disagrees this is an issue." The rationale is itself signal (false-positive rate is a Phase C input).
- `superseded` is "a newer emission subsumes this." Different from `closed`: the *addressing* didn't happen on this artifact, it happened on its successor. Aggregating closed-vs-superseded shows whether feedback emissions are stable or churny.

Collapsing them into a single terminal would lose all three signals.

## Rejected alternatives

- **Two-state (`open` / `closed`).** Rejected: erases routing, receipt, and addressing as distinct events.
- **Allow reverse transitions for "the addressing artifact was deleted" case.** Rejected: see "Why only-forward." Emit a new feedback artifact instead.
- **Single `terminated` terminal state with a reason field.** Rejected: makes the three distinct success/failure modes invisible to aggregation queries without a JOIN.
- **State machine encoded only in Rust, not in SHACL.** Rejected: the orchestrator is not the only writer in the long run (Phase D meta-loop will likely produce feedback too). Schema-side enforcement is the durable boundary.

## Consequences

**Positive:**
- Every feedback artifact carries a full audit trail of who saw it, when, and what they did.
- Phase C can compute time-to-addressed, time-to-closed, repeat-class-incidence, false-positive rate.
- Lifecycle violations surface at write time, not at SPARQL-query time.

**Negative / accepted costs:**
- The state machine is non-trivial. New contributors (and new roles) need to understand which transitions are valid.
- `core::feedback::lifecycle` is a load-bearing module; bugs there break the entire feedback flow.

**Enforcement:**
- SHACL `sh:sparql` constraints (above).
- `StreamWriter` enforces transition validity by reading prior state and checking the next-states set.
- Slice-3 TC walks each transition (`produced → routed → received → addressed → closed`) on a seeded artifact and asserts every intermediate state was reached.

## Status

Proposed. Linked to [FT-027](FT-027).
