---
id: ADR-079
title: def-ready treats open implementer feedback as Done; verify-graph-author dispatch only when no implementer work is pending
status: accepted
features:
- FT-138
supersedes: []
superseded-by: []
domains:
- api
scope: domain
content-hash: sha256:1fa12a8b9d251c7786bcb1d8955eb0393fe2532de7a8aebc00557231d3687946
---

## Context

[FT-119](FT-119) introduced `dec drive def-ready` as the Definition-of-Ready gate: a planner that classifies a feature's state and either reports `Done` (ready for shipping), `Stuck` (with a human-readable reason), or dispatches `verify-graph-author` (when a covering verification graph is missing).

[FT-108](FT-108) introduced verdict-aware defect feedback emission from the verify runner: a `rejected` aggregate verdict on a VG run emits one `dec:Feedback` per failing TC with `targetRole = "implementer"` and `lifecycleState = "produced"`. The implementer is expected to address the defect (write a CodeChange) before the loop can converge.

Witnessed failure on FT-110 (and shape-equivalent on FT-014, FT-015, FT-018):

1. VG-163 was authored for FT-110, ran, and the aggregate verdict was `rejected`.
2. FT-108's emission correctly produced an open implementer-targeted defect feedback for TC-196 (`evidence: "step 1 produced outcome fail; expected exit 0, got 101"`, `lifecycleState: "produced"`, `targetRole: "implementer"`).
3. At some later point VG-163 was superseded with a sentinel (`urn:dec:retired-stale-dogfood-...`) — a retirement / cleanup operation flagged it stale.
4. `dec drive def-ready FT-110` now classifies the covering-graph state as `Missing` (the only on-disk VG is superseded), and dispatches `verify-graph-author`.
5. The author dispatch produces nothing that changes state, the planner's cycle detector trips, and the drive reports `stuck: dispatch:verify-graph-author dispatch did not change state for FT-110`.

The structural problem: **def-ready dispatches a worker (verify-graph-author) whose entire job presupposes that no implementer work is pending**. When implementer feedback is open, the right next move is not authoring a new VG — it's letting the implementer address the defect, which will then trigger a fresh verify run via FT-100 (`auto-dispatch verify-graph-runner on CodeChange commit`), which will produce a new aggregate verdict. Re-authoring a VG ahead of that just produces a new VG that will fail the same way.

This failure mode is not rare. It fires whenever:
- A feature accumulates implementer defects faster than they're addressed.
- Stale VGs get retired (per FT-120 or manual cleanup) while implementer feedback remains open.
- A drive run leaves outstanding defects between sessions (lid-close, kill, etc.).

The broader sweep (137 features) shows the same `dispatch:verify-graph-author dispatch did not change state` pattern on at least 4 features. One systemic fix is more leverage than four feature-level workarounds.

## Decision

**Add an "open implementer feedback for the feature" check to the `FeatureReadyPlanner` classification table, with higher precedence than the `vgs_cover` (`Missing`) check. When the check fires, classify as `Done`.**

The new check answers: *does the feature have at least one open (`lifecycleState = "produced"`), implementer-targeted (`targetRole = "implementer"`), defect-class (`feedbackClass = "defect"`) `dec:Feedback` whose `sourceArtifact` is one of the feature's TCs?*

Updated classifier precedence (first match wins):

```
preflight = Warnings { gaps }              → Stuck "preflight: ADR-N unacknowledged"
any dep status ≠ complete                  → Stuck "blocked: FT-Y status=..."
spec = MissingHeading(...)                 → Stuck "spec incomplete: <heading>"
tcs = NoneLinked                           → Stuck "no TCs linked"
tcs = SomeUnready { problem_tc, ... }      → Stuck "TC quality: TC-NNN ..."
implementer feedback open for feature      → Done                                      ← NEW
vgs = Missing                              → DispatchVerifyGraphAuthor
vgs = PendingReview { graph_ids }          → Stuck "VG pending_review: VG-..."
otherwise                                  → Done
```

The decision boundary is intentional:

- The new row is positioned **after** the spec/TC-quality checks because those genuinely block all downstream work (you cannot ship a feature whose spec is missing sections or whose TCs lack runners regardless of any feedback state).
- It is positioned **before** the VG checks because the existence of open implementer feedback fundamentally invalidates the VG-missing → re-author response.
- The classification is `Done` rather than a new `Stuck "implementer-feedback pending"` row because *def-ready is about Definition-of-Ready for implementation*. Outstanding implementer defects are themselves evidence that the feature is well-defined enough for implementation work to be in flight; the next move is `dec drive ship`, not `dec drive def-ready`.

## Rejected alternatives

### Add `DispatchImplementer` action to def-ready

Make def-ready dispatch the code-writer when implementer feedback is open. Rejected — def-ready is the *gate*, not the *ship loop*. Dispatching the implementer inside def-ready muddles the responsibility split with `dec drive ship` (which does dispatch the implementer). Two driver verbs both running the same dispatch is exactly the duplication FT-110 / FT-119 split apart.

### Surface as `Stuck "implementer-feedback pending: <iri>"`

A new Stuck variant explicitly naming the open feedback IRI. Rejected — Stuck is for *blocking-on-human-authoring* states (the operator needs to write a spec section, ack an ADR, configure a TC runner). Open implementer feedback is *blocking-on-automated-work*; the operator's response is "kick off the ship loop", not "open the editor". Reporting Done makes that next move discoverable in the existing tooling without a special-case stuck row.

### Have FT-120 (stale-defect retract) close the feedback when retiring the VG

If FT-120 retracts the defect feedback whenever it retires a VG, the open-feedback row never fires for stale topology. Rejected — the feedback's claim is still valid: TC-196 *did* fail. Retiring the VG that surfaced the failure does not absolve the implementer of the work. The defect lives until a CodeChange addresses it.

### Re-author when the existing feedback is stale enough

A time-based heuristic: if implementer feedback hasn't been addressed in N hours, allow def-ready to re-author. Rejected — time is a poor proxy for human attention. The right signal is "did anyone work on this", and we already have that signal via the `addressed` lifecycle transition. The existing cycle detector covers the drive-side runaway case.

### Inspector returns a richer covering-graph state (`Rejected { graph_ids }`) and the planner branches on that

A new `CoveringGraphState::Rejected` variant. Rejected — the symptom we observed (`Missing` from `on_disk_covering_graph_present` because VG-163 is superseded) wouldn't be helped by a `Rejected` variant; VG-163 is retired by supersession, not by being marked rejected at the on-disk level. The implementer-feedback check is the actual contract the planner needs, regardless of how the VG state landed at `Missing`.

## Consequences

### Positive

- **FT-110, FT-014, FT-015, FT-018, and any feature with outstanding implementer feedback** unblock from def-ready's stuck-on-dispatch cycle.
- **Wasted verify-graph-author dispatches stop** for the open-feedback case. Each one is a non-trivial LLM call; cutting them out of the cycle saves cost and latency.
- **The planner's responsibility split crystallises**: def-ready answers "is this ready for implementation work?"; ship answers "drive it through the implementation loop." Open implementer feedback is the former's positive signal and the latter's input.

### Negative / accepted trade-offs

- **Stale implementer feedback keeps a feature classified as Done forever** if nobody runs `dec drive ship` against it. Mitigated by the existing `dec loop list` / `dec loop show` reporter (FT-109) which surfaces every feature with open defects. The operator-visible audit trail is the backstop.
- **The new inspector method requires a SPARQL query against the orchestration store on every def-ready invocation.** Latency cost is small (the query is bounded; the store fits in memory). Worth measuring if hotpaths regress.
- **`dec drive def-ready --all`'s output table changes** — features that previously reported `stuck: dispatch:verify-graph-author...` will now report `done`. Operators relying on the prior shape for triage will need to adapt; `dec loop list` is the substitute view for "what's blocking?".

### Relationship to prior decisions

- **[FT-108](FT-108)**: This ADR builds on FT-108's verdict-aware emission. The fix here doesn't change emission — it changes how the planner *responds* to existing emitted feedback.
- **[FT-119](FT-119)**: Extends the def-ready classification table with one additional row at a specific precedence position. The cycle-detection mechanism, the inspector trait, and every other piece of FT-119 are unchanged.
- **[FT-120](FT-120)**: Independent. FT-120 retracts defects whose source-VG topology no longer covers the TC. The new check here applies to non-retracted defects.

## Status

Proposed. Promotes to accepted once the implementation slice ships and a regression-guard TC asserts the new classifier row's precedence + behaviour.
