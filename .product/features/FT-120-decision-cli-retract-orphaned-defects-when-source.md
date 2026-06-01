---
id: FT-120
title: 'decision-cli: retract orphaned defects when source-VG topology no longer covers the TC'
phase: 4
status: complete
depends-on:
- FT-116
adrs:
- ADR-024
tests:
- TC-260
- TC-261
- TC-262
- TC-263
- TC-264
- TC-265
domains:
- data-model
domains-acknowledged:
  data-model: Reuses the `produced → superseded` transition declared in ADR-024. Introduces one new predicate `dec:supersededByTopologyChange` (subPropertyOf `dec:supersededBy`) to distinguish topology-change retractions from feedback-supersession retractions; no state-machine extension required.
patterns:
- PAT-001
---

## Description

FT-116 retracts open defects whose evidence has been *retracted by a
fresh approved VGR for the same graph*. It is the right mechanism for
the common case ("TC failed in VGR-N, then passed in VGR-N+1, close
the old defect"), and its closure query encodes exactly that
contract: `?vgr a dec:VerificationGraphResult ; dec:resultOf
<{graph_iri}> ; prov:wasGeneratedBy ?session`.

That contract leaves a structural blind spot: defects whose source TC
has *left the VG entirely*. A TC's runner can be migrated (e.g.
from `verify-graph` to `bash`); a TC can be unlinked from a graph; a
VG can be deactivated. In any of those cases, no future VGR will
ever land for that (graph, TC) pair — and FT-116's auto-close can
never fire. The defects stay open forever.

Just-witnessed on FT-100: 8 open defect feedbacks from VG-088
(2026-05-26) and VG-155 (2026-05-28) reference TC-162, TC-163,
TC-164. Those TCs were since migrated to `runner: bash` and no
longer participate in any VG. The planner sees
`open_defect_feedback_count > 0` → dispatches `verify-graph-author`
→ author finds nothing to do because the TCs aren't graph-verified
anymore → state hash repeats → cycle detector trips → `stuck`. The
feature is materially fixed; the orchestrator cannot tell.

The fix is the mirror of FT-116: when the verification topology
changes such that a TC's source VG no longer covers it, the open
defects from that VG against that TC are *by construction*
orphaned. They should auto-transition to `superseded` (per ADR-024,
the legal terminal for "this feedback is closed by reference, not
by addressing"), citing the topology-change detection session as the
retracting authority.

This makes the orchestrator self-healing across TC-runner
migrations, VG deprecations, and TC reassignments. Without it, every
such migration produces operator toil and direct SPARQL surgery.

## Functional Specification

### Inputs

No new operator-facing CLI flags on `dec drive` or the planner.
The behaviour is internal to the inspector / planner-pre-dispatch
path, mirroring how FT-116 hides inside the VGR-write path.

For debugging and one-shot backfill:

- `dec _retract-orphan-defects [--graph VG-NNN | --feature FT-XXX |
  --all] [--dry-run]` — hidden diagnostic CLI. Operator-driven
  backfill against an existing store. `--graph` scopes to one VG;
  `--feature` scopes to all VGs linked to a feature; `--all` sweeps
  every graph in the store. `--dry-run` lists candidates without
  writing. Companion to FT-116's `dec _retract-stale-defects`.

### Outputs

**New module**:
`crates/decision-cli/src/features/ft_120_retract_orphan_defects/`:

- `query.rs` — SPARQL queries that find candidate orphan defects.
  An orphan is a defect feedback where the source VG no longer
  has any active step verifying the TC.
- `transition.rs` — applies the `produced → superseded` (or
  `routed → superseded`) lifecycle transition per ADR-024, recording
  the closing session IRI and a textual reason on the feedback
  artifact.
- `pipeline.rs` — orchestrates the (find → filter → transition)
  flow; callable from both the diagnostic CLI and the
  planner-pre-dispatch hook.
- `cli.rs` — adapter for the `_retract-orphan-defects` diagnostic.
- `tests.rs` — unit + integration tests per the TC list.

**Planner integration** (deferred to a follow-up feature): the
inspector that computes `open_defect_feedback_count` would gain a
pre-filter step calling the orphan-retract pipeline before counting,
so the planner sees the post-retraction count and routes correctly.
This is left for a follow-up because (a) the operator-driven CLI
gets FT-100 unblocked today, and (b) the planner-side hook touches
the hot dispatch path and warrants its own design + TCs. The
pipeline is exposed via `retract_orphans_for_graph` /
`retract_orphans_for_feature` / `retract_orphans_all` precisely so
that future feature can wire it in without restructuring this code.

**Lifecycle vocabulary extension** (minor):

- New predicate `dec:supersededByTopologyChange` (subPropertyOf
  `dec:supersededBy`) — points from the superseded feedback to the
  session IRI that detected the topology change. Distinguishes
  topology-change retraction from feedback-supersession retraction,
  so audit queries can tell them apart. Mirrors FT-116's
  `dec:closedByEvidenceRetraction` (subPropertyOf `dec:closedBy`).

### State

Persists feedback lifecycle transitions in the orchestration store.
No new persistent files. On-disk VGR files and TC files are
unchanged — orphan-retraction is an orchestration-store-only edit.

### Behaviour

1. **Trigger A — planner inspect.** When the inspector enumerates
   open defects for an artifact, each candidate is checked for
   orphanhood (Step 3). Orphans are retracted in-line, then the
   open count is recomputed against the cleaned state. The planner
   sees the post-retraction count and routes correctly.
2. **Trigger B — diagnostic CLI.** `dec _retract-orphan-defects
   --graph VG-NNN` runs the same find-and-retract pass against an
   explicit graph (or feature, or store-wide for `--all`).
   `--dry-run` lists candidates without writing.
3. **Orphan check (SPARQL).** A defect feedback `<fb>` is orphaned
   iff its source VG no longer has any active step verifying its
   source TC:
   ```
   ?fb dec:sourceArtifact <tc_iri> ;
       dec:sourceSession ?session ;
       dec:lifecycleState ?state .
   ?vgr dec:resultOf <vg_iri> ; prov:wasGeneratedBy ?session .
   FILTER NOT EXISTS {
     <vg_iri> dec:steps/rdf:rest*/rdf:first ?step .
     ?step dec:providesEvidenceFor <tc_iri> .
   }
   FILTER (?state IN ("produced", "routed"))
   ```
   (`received` is excluded: once the target role has begun work on
   the feedback, the supersede-by-reference path no longer applies
   per ADR-024; the lifecycle must continue through `addressed` or
   `rejected`.)
4. **Transition write.** For each orphaned `<fb>`:
   - `<fb> dec:lifecycleState "superseded"`
   - `<fb> dec:supersededByTopologyChange <retraction_session>`
   - `<fb> dec:supersededAt <now>`
   - `<fb> dec:supersededReason "source VG <vg_iri> no longer
     covers TC <tc_iri> (topology change)"`
5. **Commit.** Transitions land atomically. The retraction session
   is recorded as a PROV-O activity so the audit trail shows who
   retracted, when, and why.
6. **Failure handling.** If a SHACL violation is raised on a
   transition (shouldn't happen — `produced → superseded` is
   declared valid by ADR-024), the entire pass rolls back. The
   diagnostic CLI reports the error; the planner-side trigger logs
   and skips the affected feedback rather than aborting the round.

### Invariants

- Defects whose source VG *still* covers the TC are NOT retracted.
  Only defects from `(vg, tc)` pairs where the VG no longer has a
  step verifying the TC qualify.
- Defects already in terminal state (`closed`, `rejected`,
  `superseded`) are NOT modified.
- Defects from non-VGR sessions (e.g. implementer or VGA-emitted
  defects whose source-session isn't a VGR) are NOT touched.
  Orphan-retraction is scoped to verifier-emitted defects only,
  matching FT-116's scoping.
- The retraction is monotonic: once superseded, never reopened. If
  a TC is later relinked to its old VG and the VG produces a new
  defect, that's a fresh feedback IRI, not a reopen.
- Planner-side retraction is idempotent: running the inspector
  twice in a row produces the same end state. The second run finds
  no candidates because the first already retracted them.
- `dec loop show FT-XXX` after retraction shows previously-open
  defects as `[superseded]` with the citation `superseded by
  topology change`.

### Error handling

- No candidates found: no-op, no error. Exit 0 for the CLI.
- A topology-check query fails (malformed VG, missing step
  predicates): skip the candidate, log a warning, continue with the
  rest. Don't abort the pass for one bad row.
- `--graph VG-NNN` against a graph that doesn't exist: error with
  exit 2 from the CLI; the planner trigger never sees this case
  because it iterates over existing defects.

### Boundaries

- This feature does NOT change how defects are emitted. The
  emission path is unchanged; only the close path gains the
  orphan-detection mechanism.
- This feature does NOT subsume FT-116. FT-116 handles
  "fresh evidence retracts stale defect" (same graph, TC flipped
  to pass). FT-120 handles "topology change orphans the defect"
  (TC left the graph). They run on different triggers against
  different SPARQL conditions.
- This feature does NOT auto-emit synthetic "topology-change
  feedback" to serve as the supersession authority. The
  retracting session is the authority; `dec:supersededBy` points
  to the session IRI via the new `supersededByTopologyChange`
  predicate. No fake feedback artifacts.
- This feature does NOT modify the FT-110 cycle detector or
  pairwise no-progress detector. They continue to read
  `open_defect_feedback_count` from the inspector; the inspector
  now produces a cleaner count by virtue of in-line retraction.
- This feature does NOT extend `dec:supersededBy` semantics
  beyond what ADR-024 already permits. ADR-024 lists
  `produced → superseded` and `routed → superseded` as legal
  terminal transitions; FT-120 uses them as-declared.

## Out of scope

- **Reopening superseded defects if the TC is later relinked to
  the VG.** The lifecycle state machine is monotonic per ADR-024;
  relinking produces a fresh feedback IRI on next verification, not
  a reopen.
- **Cross-graph orphan reconciliation.** A defect from VG-A about
  TC-X is not retracted just because TC-X also lives on VG-B. The
  query is per-(graph, TC) pair, not global.
- **TC-deletion-triggered retraction.** If a TC is deleted from
  the product graph entirely, its defects become unresolvable but
  the topology query as specified (`<vg> dec:hasStep ?step .
  ?step dec:verifies <tc_iri>`) still returns empty and so retracts
  them. A separate feature could add affirmative deletion handling;
  the MVP relies on the query naturally covering it.
- **Bulk repo-wide auto-retraction at bootstrap.** The diagnostic
  CLI supports `--all`, but no automatic sweep runs at `dec init`
  or on schedule. Operators run it when they want a sweep.
- **Operator-visible side-channel notifications.** No per-retraction
  log message on the CLI beyond a summary line. The retractions
  show up in `dec loop show` like any other lifecycle transition.
