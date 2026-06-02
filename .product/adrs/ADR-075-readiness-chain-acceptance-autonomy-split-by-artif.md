---
id: ADR-075
title: Readiness chain acceptance autonomy split by artifact kind
status: accepted
features:
- FT-127
- FT-128
- FT-129
- FT-130
- FT-131
- FT-132
- FT-133
supersedes: []
superseded-by: []
domains:
- api
- observability
scope: domain
content-hash: sha256:6b47c06514ea61facbe602db06922035868064b90c854ee0f222e4fb7f428c1b
---

**Status:** Proposed

## Context

[ADR-073](ADR-073) introduces four new authoring pairs whose interpretation
session emits a `dec:QualityVerdict` ([ADR-074](ADR-074)). An `approved`
QualityVerdict is the signal that an authored artifact is fit for downstream
use; the readiness orchestrator ([ADR-076](ADR-076)) reads `approved` verdicts
to flip readiness bits and choose the next planner action.

What's left open is **how an `approved` verdict transitions from "the judge
said yes" to "the planner observes the bit as `true`."** Three positions on
the spectrum:

- **Always human-accept** (current verify-graph-author behaviour per
  [ADR-030](ADR-030) §7 — Level-3 autonomy: worker proposes, human accepts
  before persistence).
- **Always auto-accept on `approved` verdict** (the maximum-throughput
  position; the framework trusts the judge by construction).
- **Per-kind split** — different autonomy levels for different artifact
  kinds based on the trust boundary each crosses and the observability
  available downstream.

The brief (§2.8) decides: split by artifact kind, tied explicitly to the
five-level autonomy model in
`docs/ddd/DDD_and_the_Five_Levels_of_AI_Autonomy.md`. This is the right
shape because the four authored kinds carry asymmetric trust risk:

- **TCs and VGs** are operationally narrow ("does this TC's runner-args
  point at a real test?", "does this graph step provide evidence for this
  TC?"), and their misjudgment is highly observable downstream — the
  implementer dispatch that consumes the TC, or the graph-runner that
  executes the VG, will produce a verdict of its own that surfaces drift
  within iterations.
- **feature_specs and ADRs** are operationally broad ("does this spec match
  the request's intent?", "does this ADR close the preflight gap with the
  right rationale?"), and their misjudgment may take many downstream
  iterations to surface — a thinly-authored spec produces code, the code
  passes a thinly-authored TC, and the feature ships before anyone notices
  the spec was wrong.

Authoring the criteria the feature's own code is later judged against is
the real trust boundary the brief flags (§2.4). For TC/VG the boundary is
narrow enough and observable enough to graduate now; for spec/ADR it is
not.

## Decision

**Acceptance autonomy of the four authoring pairs splits by judged artifact
kind, governed by a named fitness function and tied explicitly to the
five-level autonomy model.**

### The split

| Authored kind | Judge role | Acceptance | Autonomy level (corpus mapping) |
|---|---|---|---|
| `dec:TestCriterion` ([FT-126](FT-126) output) | tc-quality ([FT-127](FT-127)) | **Fitness-gated auto-accept** | Level 4 — auto-persist on `approved` verdict, governed by the fitness function. |
| `dec:VerificationGraph` ([FT-048](FT-048) output) | vg-quality ([FT-128](FT-128)) | **Fitness-gated auto-accept** | Level 4 — same as TC. |
| `dec:FeatureSpec` ([FT-129](FT-129) output) | spec-quality ([FT-132](FT-132)) | **Human-accept** | Level 3 — worker proposes, human accepts before persistence. |
| `dec:ADR` / `Acknowledgement` ([FT-130](FT-130) output) | adr-quality ([FT-133](FT-133)) | **Human-accept** | Level 3 — same as feature_spec. |

For comparison and continuity, the existing verify-graph-author role
([ADR-030](ADR-030)) is currently at Level 3 (human-accept on `pending_review`
graphs). The vg-quality judge introduced in [ADR-073](ADR-073) is what makes
the graduation to Level 4 possible — until there is a paired quality judgment,
auto-acceptance has no signal to trust. ADR-030's graduation criteria
("once we have empirical agreement-rate data") are exactly what this ADR's
fitness function provides.

### What "fitness-gated auto-accept" means concretely

For TC and VG quality verdicts:

1. The judge dispatch completes with an `approved` QualityVerdict.
2. The harness materialises the artifact in the product graph and flips the
   readiness bit observed by the planner — **without** a separate human-accept
   step.
3. A named cross-cutting TC under [ADR-014](ADR-014) ("readiness chain
   auto-accept agreement fitness" — [TC-315](TC-315)) runs on each verify
   pass and computes:
   - **Disagreement rate.** The fraction of auto-accepted TCs/VGs whose
     downstream consumer (implementer for TCs, graph-runner for VGs)
     produces a `rejected` or `amendment-required` verdict within the
     following N dispatches.
   - **Reopening trigger.** If disagreement exceeds a threshold over a
     rolling window of dispatches, the fitness function emits a
     blocking feedback artifact ([ADR-022](ADR-022)/[ADR-025](ADR-025)) and
     the orchestrator reverts to human-accept for that artifact kind until
     a human-driven amendment lowers the rate. The downgrade is itself
     auditable: a `dec:AutonomyDowngrade` event tied to the offending kind
     and the rolling-window evidence.

The fitness function is a first-class artifact (per [ADR-014](ADR-014) §
"Rules are ADRs / Checks are TCs"). Threshold values (disagreement rate,
rolling-window size, reopening trigger) are operator-tunable through
[ADR-068](ADR-068)'s config substrate under a new `[verification.autonomy]`
section. Default thresholds are conservative; the brief (§2.4) accepts the
trust boundary so the defaults can graduate over time without requiring an
ADR amendment per knob.

### What "human-accept" means concretely

For spec and ADR quality verdicts (Slice B):

1. The judge dispatch (spec-quality [FT-132](FT-132) or adr-quality
   [FT-133](FT-133)) completes with an `approved` QualityVerdict.
2. The harness records the verdict but does NOT flip the readiness bit; the
   proposed artifact sits in a `pending_review` state mirroring
   [ADR-030](ADR-030) §7's pattern for graph proposals.
3. A human operator runs an inspection command (`dec drive show FT-XXX`
   reports the pending proposal alongside its verdict) and either:
   - Accepts via `dec drive accept --artifact <proposal-iri>` — the
     readiness bit flips and the planner picks up the chain.
   - Rejects via `dec drive reject --artifact <proposal-iri> --reason …` —
     the proposal is superseded; the planner re-dispatches the author with
     the rejection as bundle context.
4. The MCP twin mirrors the CLI surface as an inspection-then-acceptance
   pair, per [ADR-029](ADR-029)'s CLI/MCP discipline.

Auto-accept is **not** plausibly safe for spec/ADR today: the trust boundary
is too broad and the observability is too slow. Graduation criteria are
explicit (see "Future graduation" below).

### Autonomy-level mapping

The five-level model in
`docs/ddd/DDD_and_the_Five_Levels_of_AI_Autonomy.md` distinguishes:

- **L3** (Conditional): worker proposes; human accepts before persistence.
- **L4** (High): worker persists autonomously; system measures and reverts
  on drift.
- **L5** (Full): worker persists autonomously without revertable safety net.

The decision lands TC/VG at L4 and spec/ADR at L3. L5 is explicitly out of
scope for this ADR — even fitness-gated auto-accept is a recoverable
position; full L5 requires a stable cross-role agreement metric we do not
have data for yet ([ADR-021](ADR-021)).

### Future graduation

Graduation criteria, recorded here so subsequent ADRs amend rather than
relitigate:

- **TC and VG to L5.** Requires the fitness function below the
  reopening-trigger threshold for ≥ 90 days of continuous operation across
  the value-stream's active features. ADR amend.
- **feature_spec and ADR to L4.** Requires (a) the QualityVerdict
  disagreement rate measured on human-accept rounds to stabilise below a
  threshold; (b) the per-kind judge workers (spec-quality / adr-quality)
  have accumulated their own agreement-rate track record over a comparable
  rolling window. The judge roles are now fixed (per [ADR-073](ADR-073) §
  "Why four judge roles, not fewer"); graduation is a function of data, not
  of role consolidation.

### What the planner reads

[ADR-076](ADR-076)'s planner reads readiness via a uniform predicate
(`complete` DispatchGroup + `approved` QualityVerdict). This ADR adds the
acceptance layer between the predicate and the planner's read:

- TC/VG: the readiness bit flips on the verdict landing in `complete` status.
- spec/ADR: the readiness bit flips on the human-accept event, not on the
  verdict landing.

The planner doesn't know which path applies — it reads the bit. The
distinction is in the harness's transition from "verdict approved" to
"readiness bit flipped." Per-kind autonomy is a **harness** concern, not a
planner concern.

## Rejected alternatives

- **Always human-accept (status quo extended to all four kinds).** Rejected
  per §2.4 and §2.8: the brief explicitly notes that authoring TCs/specs is
  in-scope LLM work; pre-gating every TC with human review is the failure
  mode the readiness orchestrator is designed to address. The TC/VG arms
  are operationally narrow enough that L4 with a fitness function is the
  right starting point.
- **Always auto-accept (all four kinds at L4).** Rejected per §2.8: the
  trust boundary for spec/ADR is too broad and the observability is too
  slow. A thinly-authored spec affects every downstream artifact in the
  feature; the cost of getting it wrong amortises over the whole chain.
- **Same autonomy level for all four, but a *higher* one (L5).** Rejected:
  no fitness data exists to certify L5. The corpus is explicit that L5 is
  graduated to, not granted.
- **Per-feature opt-in (a feature_spec frontmatter flag).** Rejected: makes
  the autonomy decision per-artifact rather than per-kind, fragmenting the
  fitness function's data. Operators can override with `--no-author` (the
  planner's escape hatch, [ADR-076](ADR-076)) or per-environment config
  ([ADR-068](ADR-068)).
- **Make auto-accept the default and require an opt-out for human-accept.**
  Rejected for spec/ADR per the trust boundary above; accepted for TC/VG
  as exactly what this ADR proposes.
- **No fitness function; trust the judge by construction.** Rejected: the
  fitness function is the safety net that makes L4 a graduation from L3
  rather than a leap. Without it the framework has no way to catch judge
  drift and the L4 position is not reversible without manual intervention.

## Consequences

**Positive:**

- TC/VG auto-acceptance unblocks the readiness orchestrator's natural
  rhythm: tc-author produces drafts, tc-quality approves, planner moves on
  — same shape as the implementer/verifier pair today.
- spec/ADR human-accept preserves the trust boundary where it matters most.
- The fitness function makes L4 reversible. Disagreement bursts get
  detected and the system downgrades to L3 automatically.
- Autonomy levels are graph-resident artifacts ([ADR-014](ADR-014) pattern),
  not buried constants; the corpus's five-level model has a mechanical
  carrier in the orchestration store.

**Negative / accepted costs:**

- The fitness function is a new measurement surface that has no historical
  data. Default thresholds will need tuning. Recommend starting with a
  conservative reopening trigger (e.g. disagreement > 20% over rolling
  window of 20 dispatches) and tuning as data accumulates.
- spec/ADR pending-review queues are a new operator-visible state that
  requires inspection-and-accept UX (a CLI surface). Slice B owns the UX.
- A spec or ADR proposal sitting in pending-review blocks the readiness
  chain for that feature until acted on. This is correct (the trust
  boundary is what's being preserved) but means human latency is on the
  critical path for the affected features. Operators should treat pending
  proposals as a high-priority inbox.

**Enforcement:**

- The fitness function is [TC-315](TC-315) validated by this ADR, runner
  `runner: bash` against `scripts/checks/readiness-autonomy-agreement.sh`
  (authored alongside this ADR cluster).
- Per-kind autonomy is enforced in the harness's `quality_verdict_accepted`
  state transition; [TC-314](TC-314) asserts the routing (TC/VG verdicts
  auto-flip the readiness bit; spec/ADR verdicts only flip on human-accept).
- A cross-cutting TC asserts no `dec:AutonomyDowngrade` event exists with
  an unaddressed underlying disagreement — i.e. the downgrade-revert flow
  exits.

## Status

Proposed. Linked to [ADR-073](ADR-073) (the pair lifecycle that produces
the verdicts), [ADR-074](ADR-074) (the verdict class), and
[ADR-076](ADR-076) (the planner that reads the bit). Fitness function and
threshold configuration ride [ADR-068](ADR-068)'s config substrate.
