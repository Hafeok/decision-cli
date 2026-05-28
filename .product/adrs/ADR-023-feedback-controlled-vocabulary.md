---
id: ADR-023
title: Feedback controlled vocabulary
status: proposed
features: []
supersedes: []
superseded-by: []
domains: []
scope: cross-cutting
source-files:
- crates/decision-cli/src/core/feedback/class.rs
---

## Context

[ADR-022](ADR-022) makes feedback a first-class flow class. The class vocabulary is the discriminator workers use when emitting feedback and the routing table ([ADR-026](ADR-026)) uses when deciding where to send it. Drift in the vocabulary — workers inventing new classes, the routing table not recognizing them — collapses the routing layer.

The vocabulary needs to be:

- **Small** — workers must reliably pick one class per emission.
- **Disjoint** — every observable emergent-decision pattern fits exactly one class.
- **Stable** — adding a class is a schema migration with an ADR amendment, not a runtime choice.

`Implementing_DDD.md` §6 names the four primary classes informally: `gap`, `contradiction`, `unimplementable`, `scope-issue`. This ADR adopts those and adds two more discovered during slice-1 scoping: `defect` (the action produced something wrong that the action itself caught post-hoc) and `capability-request` (the action needed a tool or interface that doesn't exist).

## Decision

**The `dec:feedbackClass` vocabulary is six values, enforced by SHACL `sh:in`. New classes require a schema migration recorded as an ADR amendment.**

| Class | Trigger from the action's perspective | Default target role | Default blocking |
|---|---|---|---|
| `gap` | "The bundle does not contain enough information for me to act with confidence." | spec-author (slice 3: the feature_spec's author role; for decision-cli's own dispatches, the human until Phase D) | blocking |
| `contradiction` | "Two artifacts in my bundle disagree on a load-bearing point." | architect (slice 3: routed to a designated human reviewer until the architect role lands in Phase B) | blocking |
| `unimplementable` | "What I'm being asked to produce is not producible with the tools and inputs I have." | spec-author | blocking |
| `scope-issue` | "The feature_spec has drifted beyond the slice's stated bounds; doing all of it would violate the slice plan." | spec-author | non-blocking (the action proceeds on the in-scope portion, flags the rest) |
| `defect` | "I produced something just now and on review I notice a class of error worth surfacing — even though my own verdict (if any) is success." | verifier or self | non-blocking |
| `capability-request` | "I needed a tool / API / artifact type that doesn't exist. The work is paused or worked around; the long-term answer is to author the missing capability." | architect or platform-owner | non-blocking |

### Class definitions in detail

**`gap`** — *Missing information.* The action determines the bundle lacks specificity to proceed safely. The action does NOT guess; it emits `gap` and pauses (blocking). The addressing artifact is typically an amended feature_spec or a new ADR. Example: implementer asked to wire a verifier subscription, but the routing-table semantics for "no compatible role found" aren't specified anywhere — gap.

**`contradiction`** — *Inconsistent inputs.* The action finds two artifacts in its bundle (two ADRs, an ADR and a feature_spec, two TCs) that disagree on a point the action must resolve. Action does NOT pick one; emits `contradiction` (blocking). Addressing artifact: an ADR amendment or supersession recording the resolution. Example: ADR-005 says "every artifact carries `dec:inStream`" but a Phase A feature_spec mentions a stream-less metric artifact — contradiction.

**`unimplementable`** — *Tools or inputs insufficient.* Distinct from `gap`: a gap can be filled with more spec; an unimplementable cannot. Example: the verifier worker needs a structured-output guarantee from the LLM that the current model binding doesn't provide. Addressing artifact: a feature_spec for the missing capability (which often loops back as a `capability-request`).

**`scope-issue`** — *Out-of-bounds work.* The action notices the spec is asking for behavior outside the slice's declared bounds. Non-blocking by default: the action does what is in-scope and reports the out-of-scope tail. Example: the verifier feature_spec includes a metric dashboard the slice plan defers to Phase C — scope-issue.

**`defect`** — *Self-discovered error.* Post-production, the action recognizes a class of mistake (off-by-one, wrong identifier convention, naming inconsistency). Non-blocking: the action might still emit the artifact (or amend it) and flag the defect for downstream review. Distinct from a verifier `rejected` verdict — `defect` comes from the action's own retrospection.

**`capability-request`** — *Missing tool.* The action proceeded with a workaround but flags the missing capability as a candidate for explicit authoring. Non-blocking: the work shipped; the feedback is a signal for the next planning round. Example: workers want a `read_adjacent_file` tool to validate references in produced code; current bundle-in / artifact-out contract forbids it ([ADR-008](ADR-008)).

### Disjointness rules

- A gap that could be filled with more spec is `gap`, not `unimplementable`.
- A spec self-contradiction is `contradiction`, not `gap`.
- An out-of-bounds *addition* is `scope-issue`; an out-of-bounds *requirement that makes the in-scope work impossible* is `unimplementable`.
- A defect the verifier catches is a `rejected` verdict; a defect the action catches itself is a `defect` feedback emission.

### SHACL fragment

```turtle
@prefix dec: <https://decision-cli.dev/ns#> .
@prefix sh:  <http://www.w3.org/ns/shacl#> .

dec:FeedbackClassShape a sh:NodeShape ;
    sh:targetClass dec:Feedback ;
    sh:property [
        sh:path dec:feedbackClass ;
        sh:in ( "gap" "contradiction" "unimplementable"
                "scope-issue" "defect" "capability-request" ) ;
        sh:minCount 1 ; sh:maxCount 1 ;
    ] .
```

### Adding a class

A seventh class requires:

1. An ADR amendment to this ADR with rationale.
2. The SHACL `sh:in` list extended in `core/ontology/`.
3. The routing table ([ADR-026](ADR-026)) extended with a default target.
4. Worker SDKs ([FT-031](FT-031)) updated to accept the new class.
5. Documentation in `Implementing_DDD.md` §6 if it changes the framework's vocabulary.

All five land atomically in one request per `.product/requests.jsonl`.

## Rejected alternatives

- **Free-form `feedbackClass: string`.** Rejected: prevents routing, prevents aggregation, invites worker prose drift.
- **Three classes only (`gap`, `contradiction`, `unimplementable`).** Rejected: collapses important distinctions. `scope-issue` and `defect` are observably different from the other three.
- **Twelve classes.** Rejected: worker decision burden too high; reliability of class-picking drops with cardinality.
- **Hierarchical classes (e.g. `gap.missing-edge-case`, `gap.missing-rationale`).** Rejected for Phase A: hierarchy adds matching complexity to the routing table without clear payoff. Revisit if class incidence data (Phase C) shows hierarchical patterns.

## Consequences

**Positive:**
- Routing is deterministic from `feedbackClass` + `target`.
- Aggregation queries are straightforward (e.g. "all gap feedback for FT-007").
- The vocabulary is small enough to be promptable to a worker LLM with high reliability.

**Negative / accepted costs:**
- Six classes is a non-trivial discriminator. Slice 3 needs explicit prompt-engineering and probably a small calibration corpus.
- Adding a class is a schema migration. This is by design (stability) but means new patterns of feedback require deliberation.

**Enforcement:**
- SHACL `sh:in` at write time.
- A slice-3 TC asserts each of the six classes can be emitted and routed through the system with no manual intervention.

## Status

Proposed. Linked to [FT-028](FT-028).
