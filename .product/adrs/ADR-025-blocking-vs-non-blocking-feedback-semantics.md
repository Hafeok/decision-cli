---
id: ADR-025
title: Blocking vs non-blocking feedback semantics
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: cross-cutting
content-hash: sha256:5d20a2efe3b5ec7d8e275fec5020908cd34e32edc22537047c622b6e5996750c
source-files:
- crates/decision-cli/src/core/dispatch/lifecycle.rs
- scripts/checks/feedback-blocking-pauses.sh
---

## Context

Some feedback must stop the world; some must not. The implementer encountering a *gap* cannot proceed responsibly — guessing means producing wrong code under the appearance of success. The implementer noticing a *capability-request* (a future-tense "we'd like a tool for this") has no reason to stop — the work shipped, the signal is for planning.

[ADR-023](ADR-023) tags each class with a default blocking disposition. This ADR codifies *what blocking actually means* — at the dispatch lifecycle, at the worker, at the orchestrator — and the rules for when the default applies vs. when an explicit override is justified.

## Decision

**Feedback is either blocking or non-blocking. Blocking feedback pauses the emitting dispatch and refuses to advance until the feedback transitions to `addressed` or `rejected`. Non-blocking feedback emits in parallel; the emitting dispatch proceeds. The disposition is a property of the *emission*, not the class — the class has a default but each emission can override with rationale.**

### Blocking semantics

When a worker emits feedback with `blocking: true`:

1. The worker's `emit_feedback` call returns *and the worker is expected to exit* (the action is structurally aborted). Workers that emit blocking feedback do not produce a normal action artifact in the same session; the feedback IS the session's primary output.
2. The orchestrator transitions the emitting dispatch to `paused-for-feedback`. The `DispatchGroup` ([ADR-017](ADR-017)) does not enter `awaiting-interpretation` — there's nothing to interpret yet.
3. The orchestrator routes the feedback per [ADR-026](ADR-026) and waits.
4. When the feedback reaches `addressed` (the target role produced an addressing artifact) or `rejected`:
   - `addressed`: the orchestrator dispatches a *retry* action session for the original work, with the addressing artifact added to the bundle. The original action session's terminal status becomes `superseded-by-retry`. The `DispatchGroup` lifecycle continues with the retry.
   - `rejected`: the orchestrator transitions the original dispatch to `feedback-rejected-action-blocked`. The retry does not auto-fire. Operator must intervene (or, in Phase D, the meta-loop produces a different decision).

### Non-blocking semantics

When a worker emits feedback with `blocking: false`:

1. The worker's `emit_feedback` call returns immediately; the worker continues toward producing its normal action artifact.
2. The orchestrator routes the feedback per [ADR-026](ADR-026) in parallel with the ongoing dispatch.
3. The emitting dispatch proceeds through its normal action → interpretation → complete lifecycle. The feedback's own lifecycle runs independently.
4. Phase C aggregation correlates: dispatches whose non-blocking feedback ever reaches `closed` produce a "the action shipped *and* the system improved" record.

### Default-disposition mapping (mirrors [ADR-023](ADR-023))

| Class | Default disposition |
|---|---|
| `gap` | blocking |
| `contradiction` | blocking |
| `unimplementable` | blocking |
| `scope-issue` | non-blocking |
| `defect` | non-blocking |
| `capability-request` | non-blocking |

### Override rules

The worker may override the default in either direction *with rationale*:

- **Blocking → non-blocking override** (e.g. "the gap is in a non-critical edge case; I proceeded with a documented assumption"). Allowed if the worker can articulate the assumption and the assumption is captured in the produced artifact. The feedback still emits, just doesn't pause.
- **Non-blocking → blocking override** (e.g. "this defect, if I ship it, makes downstream dispatches fail"). Allowed; rare.

Overrides are recorded as `dec:dispositionOverride` on the feedback artifact with the rationale. Phase C aggregates override patterns: a class whose default is consistently overridden is a signal that the default is wrong (an ADR-023/ADR-025 amendment candidate).

### What blocking does NOT do

- It does **not** pause other dispatches. Only the *emitting* dispatch pauses. Concurrent dispatches against other features proceed normally.
- It does **not** pause the verifier or other roles that might process unrelated work.
- It does **not** create a thread / process wait. The orchestrator's dispatch lifecycle is event-driven; "paused" is a terminal-pending state in the store, not a held thread.

### Phase A constraint

Slice 3 implements the lifecycle and the disposition machinery. The *automated retry* on `addressed` is the riskiest part — it requires the bundle assembler to accept the addressing artifact as an additional input. Phase A's scope includes the retry path *only for `gap` feedback addressed by feature_spec amendments* — i.e. the simplest case. Other class/disposition combinations land their retry semantics in later slices; until then, operator-triggered re-dispatch is the resumption mechanism.

### Concurrency note

If a single dispatch emits multiple feedback artifacts (some blocking, some not), the dispatch pauses if ANY are blocking. The dispatch resumes only when ALL blocking feedback is `addressed` or `rejected`. Non-blocking feedback emitted alongside blocking does not extend the pause — they're routed independently.

## Rejected alternatives

- **All feedback is blocking.** Rejected: makes the implementer's signal-to-noise ratio terrible. A capability-request shouldn't stop the world.
- **All feedback is non-blocking; "blocking" is just a tag for prioritization.** Rejected: a gap that gets silently dropped means the implementer guesses and produces wrong work.
- **Disposition is a property of the class only (no override).** Rejected: defaults are calibrated for the median case; specific emissions know better. Phase C needs the override signal to recalibrate defaults.
- **Disposition is a property of the receiver, not the emitter.** Rejected: the emitter has the most context about whether they can proceed without resolution. The receiver doesn't know.

## Consequences

**Positive:**
- The implementer never has to guess about a gap — the protocol gives it an out.
- Non-blocking feedback flows in parallel, so the framework's throughput isn't gated on every signal.
- Override telemetry tells Phase C when the defaults are miscalibrated.

**Negative / accepted costs:**
- The dispatch lifecycle gains a `paused-for-feedback` state and an automated-retry path that must be implemented carefully.
- Workers gain a non-trivial decision: blocking or not. Slice 3's worker SDK provides the default per class; the override path requires explicit rationale.

**Enforcement:**
- The dispatch lifecycle state machine refuses to advance a `paused-for-feedback` dispatch until all blocking feedback is terminal.
- A slice-3 TC asserts: a worker emitting blocking feedback prevents `dec implement FT-XXX` from exiting 0 until the feedback is addressed; a worker emitting non-blocking feedback allows `dec implement FT-XXX` to complete normally.

## Status

Proposed. Linked to [FT-032](FT-032).
