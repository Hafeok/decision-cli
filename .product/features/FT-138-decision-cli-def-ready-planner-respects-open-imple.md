---
id: FT-138
title: 'decision-cli: def-ready planner respects open implementer feedback before dispatching verify-graph-author'
phase: 4
status: planned
depends-on: []
adrs:
- ADR-079
- ADR-072
tests:
- TC-345
- TC-346
- TC-347
- TC-348
- TC-349
domains:
- api
domains-acknowledged:
  observability: FT-138 ships 4 TCs (TC-345 exit-criteria + TC-346/347/348 scenarios) satisfying ADR-072. ADR-072 spans api + observability; api is the primary domain. Observability concerns are covered by TC-348 (integration test asserts classifier behaviour against live orchestration-store state via production inspect_dor wiring — surfaces real planner output). Explicit acknowledgement per ADR-072 review gate.
---

## Description

The implementation slice for [ADR-079](ADR-079)'s decision to add an "open implementer feedback for the feature" check to `FeatureReadyPlanner`'s classification table, with higher precedence than the `vgs_cover` check, classifying as `Done` when it fires.

Witnessed motivating failure: `dec drive def-ready FT-110` cycles on `stuck: dispatch:verify-graph-author dispatch did not change state for FT-110` because VG-163 (the previously-failing VG) is superseded by a sentinel, the on-disk-VG check returns `Missing`, and the planner dispatches verify-graph-author despite an open implementer-targeted defect feedback (`<urn:dec:feedback:8f0504b1-...>`, `lifecycleState: "produced"`, `sourceArtifact: TC-196`).

The slice is small and surgical: one new inspector trait method, one new classifier row at a specific precedence position, four TCs (precedence + positive + negative + integration). No changes to FT-108's emission, FT-120's retraction, or the executor.

One subcommand → one slice — no new CLI surface; the change is internal to `features/ft_119_drive_def_ready/`.

## Functional Specification

### Inputs

- The current `FeatureReadyPlanner` in `crates/decision-cli/src/features/ft_119_drive_def_ready/planner.rs` (classifier table at module top + `classify_and_hash` impl).
- The current `GraphInspector` trait + `inspect.rs` default impl + `inspect_dor.rs` production wiring.
- A fixture orchestration store with at least one `dec:Feedback` artifact (`produced`, `defect`, `implementer`) whose `sourceArtifact` is a known TC, to exercise the new row.

### Outputs

- `GraphInspector` trait gains `has_open_implementer_feedback_for_feature(feature_id) -> Result<bool, InspectError>`.
- Default impl in `inspect.rs` returns `Ok(false)` (matches the test-stub pattern of every other inspector method).
- Production impl in `inspect_dor.rs` runs a SPARQL query over the orchestration store filtering by `feedbackClass = "defect"`, `lifecycleState = "produced"`, `targetRole = "implementer"`, and `sourceArtifact ∈ feature.tcs`.
- `FeatureReadyPlanner::classify_without_cycle_check` (or its tail equivalent) gains one new check between the `tcs` block and the `vgs` block: if the new inspector method returns `true`, return `Action::Done`.
- The state-hash function (`state_hash_for_feature` / `classify_and_hash`) folds in the new inspector signal so cycle detection considers open-feedback as part of feature state (a feedback going from `produced` → `addressed` between iterations changes the hash).
- Three new unit tests in `planner.rs::tests` plus one new integration test in `crates/decision-cli/tests/`.

### State

- Updated on-disk: `crates/decision-cli/src/features/drive/inspect.rs` (trait method + default impl), `crates/decision-cli/src/features/drive/inspect_dor.rs` (production impl + SPARQL), `crates/decision-cli/src/features/ft_119_drive_def_ready/planner.rs` (classifier table doc + `classify_and_hash` body + state hash), `crates/decision-cli/src/features/ft_119_drive_def_ready/dispatch_tests.rs` (any FinalizeInput-equivalent literals).
- Updated on-disk (tests only): existing `FeatureReadyPlanner` unit tests that construct a `StubInspector` (or equivalent) get the new method added with default `false`.
- Preserved on-disk: every other module; FT-108's emission code; FT-120's retraction; the executor; the run loop; the CLI surface.
- No graph migration; no orchestration-store schema change; no on-disk artifact change.

### Behaviour

#### Phase 1 — Extend the inspector trait

1. Add `fn has_open_implementer_feedback_for_feature(&self, feature_id: &str) -> Result<bool, InspectError>` to the `GraphInspector` trait in `inspect.rs`.
2. Default impl returns `Ok(false)` (matches `tcs_linked_state_for_feature`, `dependency_statuses_for_feature`, etc. — the trait's "test-friendly defaults that say nothing's wrong" pattern).
3. Existing test stubs (in any `tests/` module that implements `GraphInspector`) compile unchanged because they pick up the default.

#### Phase 2 — Production implementation in `inspect_dor.rs`

1. Add `pub(super) fn has_open_implementer_feedback(workdir, product_root, feature_id) -> Result<bool, InspectError>`.
2. Resolve the feature's TC short ids via `resolve_feature_tcs_short(product_root, feature_id)` — the same helper `on_disk_covering_graph_present` already uses.
3. Build TC IRIs (`tc_iri_for(short)`).
4. Run a SPARQL `ASK` query:
   ```sparql
   PREFIX dec: <https://decision-cli.dev/ns#>
   ASK {
     GRAPH ?g {
       ?fb a dec:Feedback ;
           dec:feedbackClass "defect" ;
           dec:lifecycleState "produced" ;
           dec:targetRole "implementer" ;
           dec:sourceArtifact ?tc .
       FILTER(?tc IN (<tc_iri_1>, <tc_iri_2>, ...))
     }
   }
   ```
5. Return the boolean.
6. Wire it into the production `GraphInspector` impl as the override for the trait method.

#### Phase 3 — Classifier row

1. Update the module-top documentation table in `planner.rs` to add the new row at the documented precedence.
2. In `classify_and_hash` (or whichever function carries the first-match-wins ladder), after the TC checks and before the VG checks, call `self.inspector.has_open_implementer_feedback_for_feature(feature_id)?`. If `Ok(true)`, return `Action::Done`.
3. **Precedence vs `vgs = PendingReview`** — the new row is intentionally positioned **above** BOTH `vgs = Missing` AND `vgs = PendingReview`. Rationale: open implementer feedback is evidence that the implementer's loop has work pending right now — that work invalidates whatever a human accept/reject decision on a pending VG would conclude. Once the defect is addressed, the next verify run produces a fresh verdict, which informs whether the pending VG accept is still meaningful. Returning `Done` immediately tells the operator "go to `dec drive ship`"; surfacing the pending VG as `Stuck` would be a misleading instruction. TC-346 locks the precedence-over-`Missing` half; the precedence-over-`PendingReview` half is documented here and exercised opportunistically by TC-348's fixture (a superseded VG with no live pending session, but the implementer is free to add an extra `StubInspector` permutation under TC-346 if they want to lock it explicitly).
4. The state-hash for the iteration folds in the boolean (e.g. as an extra `0u8 / 1u8` hashed alongside the existing dimensions). This means a defect transitioning from `produced` → `addressed` between iterations changes the hash, preventing false-positive cycle detection across the lifecycle transition. **TC-349 is the silent-regression guard** for this property — an implementer who only adds the classifier row but forgets the hash update would still pass TC-345/346/347/348 and only break things in live drives.

#### Phase 4 — Tests

1. **TC-345 (exit-criteria)** — Precedence: a fixture inspector returning `tcs = SomeUnready` AND `has_open_implementer_feedback = true` AND `vgs = Missing` produces `Stuck "TC quality: ..."` (the TC check still wins; the new row is below it).
2. **TC-346** — Positive: a fixture inspector returning `tcs = AllReady`, `has_open_implementer_feedback = true`, `vgs = Missing` produces `Action::Done`. The new row fires before the VG check.
3. **TC-347** — Regression: a fixture inspector returning `tcs = AllReady`, `has_open_implementer_feedback = false`, `vgs = Missing` produces `Action::DispatchVerifyGraphAuthor` (the new row does NOT fire; existing behaviour preserved).
4. **TC-348** — Integration: a tempdir-backed orchestration store with a real defect-class implementer-targeted `produced` feedback artifact for an FT-X TC, plus a feature spec linking that TC and a superseded VG. Running `FeatureReadyPlanner::classify` via the production `inspect_dor::*` wiring returns `Action::Done`.
5. **TC-349** — State-hash regression guard: two `classify_and_hash` calls differing only in `has_open_implementer_feedback_for_feature`'s return value produce different state hashes. Without this property the cycle detector false-positives across `produced → addressed` lifecycle transitions; the implementer must fold the new boolean into the hash.

### Invariants

- **Existing precedence preserved.** Every prior classifier row continues to fire for the same inputs; the new row is *only* additive.
- **Cycle detector still works.** The state hash includes the new inspector signal; a feedback transitioning state changes the hash; the buffer-based detector doesn't false-positive across legitimate transitions.
- **No worker code changes.** Workers (code-writer, verify-graph-author) are oblivious to the new check.
- **No emission changes.** FT-108's path is unchanged; the planner only consumes feedback, never emits it.
- **No retraction changes.** FT-120's stale-defect-retract logic is unchanged.
- **No `dec drive ship` changes.** Ship's planner has its own classification table for the implementer dispatch; this slice doesn't touch it.

### Error handling

- **SPARQL query failure** (store I/O error, malformed bench/feature IRIs) → return `Err(InspectError::Store { detail })`; planner surfaces as a PlanError. Matches existing error-flow for `aggregate_verdict_for_feature` etc.
- **Feature has no linked TCs** (`resolve_feature_tcs_short` returns empty) → return `Ok(false)`. No TCs means no possible sourceArtifact match; safe default.
- **Orchestration store missing** (fresh `.dec/` without any sessions) → return `Ok(false)`. The store-load helpers tolerate absence and the SPARQL ASK over an empty store returns false.

### Boundaries

- **In scope.** Four phases above; one new inspector trait method; one new production impl with SPARQL; one new classifier row at documented precedence; four TCs covering precedence + positive + regression + integration. Documentation update to the planner module's top-comment table.
- **Out of scope.** Changes to `ship`'s planner (separate classifier; not affected by this fix). Changes to FT-108's emission logic. Changes to FT-120's retraction. Changes to the executor or run loop. A new `Stuck` variant for "implementer-feedback pending" (rejected in ADR-079 §Rejected alternatives). A `DispatchImplementer` action on def-ready (also rejected). Time-based heuristics (rejected). Exposing the open-feedback count in the planner's row output (a UX call for a separate slice).

## Out of scope

- Changes to `dec drive ship`'s planner.
- Changes to FT-108's emission paths.
- Changes to FT-120's retraction.
- Executor / run-loop changes.
- New Stuck variants.
- A new DispatchImplementer action under def-ready.
- Time-based stale-feedback heuristics.
- Feedback-count display in `dec drive def-ready --all` output.
- Backfilling closed feedback for prior failed drives.
