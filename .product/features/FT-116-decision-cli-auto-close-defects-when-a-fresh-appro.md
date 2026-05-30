---
id: FT-116
title: 'decision-cli: auto-close defects when a fresh approved VGR retracts the failing evidence'
phase: 4
status: complete
depends-on: []
adrs: []
tests:
- TC-239
- TC-240
- TC-241
- TC-242
- TC-243
- TC-244
- TC-245
domains:
- data-model
domains-acknowledged:
  data-model: Extends the feedback lifecycle vocabulary with one new predicate (dec:closedByEvidenceRetraction). The data-model-domain ADRs (graph-as-state, PROV-O lineage, lifecycle SM in ADR-024) govern the transition discipline; no new ADR-level decision.
patterns:
- PAT-001
---

## Description

The verifier's lifecycle state machine treats defect feedback
as monotonically open until a worker explicitly transitions it
to `closed` via `addressed_feedback_iris`. That assumption
holds for defects fixed by an implementer's code change — but
not for defects whose underlying evidence is retracted by a
later approved VGR.

Just-witnessed on FT-112: VGR-575 returned `verdict
"approved"` for VG-167 at 14:24. Every step passed. But 5
defects from VG-167's earlier failing runs were still in
`produced` lifecycle state. The planner's
`open_defect_feedback_count` reads those as open work →
dispatches implementer → implementer can't reproduce the
failures because the steps now pass → 30 minutes of LLM
budget burned chasing phantom defects until the per-dispatch
timeout fired.

The fix is mechanical: when a new VGR lands with
`outcome="pass"` for a TC, any open defect emitted by a prior
VGR of the *same graph* against the *same TC* is by
construction retracted — fresh evidence supersedes stale
evidence. The harness should auto-transition those defects to
`closed` at VGR-write time, citing the new VGR as the
retracting authority.

This is a write-time fix in the verifier's VGR-persistence
path. The planner stays simple; the lifecycle state stays
monotonic; the cycle detector and pairwise no-progress
detector both see correct open counts.

A first-class side benefit: FT-110's cycle detector becomes
more sensitive. Today's "0 open after auto-close" state
becomes reachable by mechanical evidence rather than only by
worker action, so cycles that involve "stale defects keep
dispatch firing" terminate one round earlier.

## Functional Specification

### Inputs

No new CLI flags. The behaviour is internal to the verifier's
VGR-write transaction in the harness; operators see the
effect indirectly through `dec loop show FT-XXX` (fewer open
defects, more `closed` entries citing VGR-NNN as the closer).

For debugging:

- `dec _retract-stale-defects --graph VG-NNN [--dry-run]` —
  hidden one-shot CLI that runs the auto-close pass against
  an existing graph's latest VGR. Operator-driven backfill
  for graphs whose VGRs landed before FT-116 shipped.

### Outputs

**New module**:
`crates/decision-cli/src/features/ft_116_retract_stale_defects/`:

- `query.rs` — SPARQL queries that find candidate stale
  defects for a given (graph, newly-approved VGR) pair.
- `transition.rs` — applies the `closed` lifecycle transition
  per ADR-024, recording the closing VGR IRI and a textual
  reason on the feedback artifact.
- `pipeline.rs` — orchestrates the (find → filter → transition)
  flow; called by the verifier's VGR-write path.
- `cli.rs` — adapter for the `_retract-stale-defects`
  diagnostic.
- `tests.rs` — unit + integration tests per the TC list.

**Harness integration**: the existing
`core::verify::result::persist_vgr` (or equivalent) gains a
post-commit hook that invokes
`retract_stale_defects(workdir, store, vgr_iri)`. The hook
runs *after* the VGR quads are committed so the find query
sees the new evidence, but *inside* the same StreamWriter
transaction so the lifecycle transitions are atomic with the
VGR landing.

**Lifecycle vocabulary extension** (minor):

- New predicate `dec:closedByEvidenceRetraction` (subPropertyOf
  `dec:closedBy`) — points from the closed feedback to the
  retracting VGR. Distinguishes mechanical evidence-driven
  closure from worker-driven `addressed_feedback_iris`
  closure, so future tooling can tell them apart in audit
  trails.

### State

Persists feedback lifecycle transitions in the orchestration
store. No new persistent files. The on-disk VGR files
(`.dec/verify/result/VGR-NNN.ttl`) are unchanged — the
auto-close happens at the orchestration-store layer only.

### Behaviour

1. **VGR-write trigger.** Verifier finishes a run, has
   composed VGR-N with one EvidenceProjection per step. Begins
   the StreamWriter transaction that commits VGR-N's quads.
2. **After VGR-N is committed inside the same transaction**,
   the auto-close pass runs:
   a. For every EvidenceProjection in VGR-N with
      `outcome = "pass"`, capture (tc_iri, graph_iri, vgr_iri).
   b. SPARQL query: find open defect feedback where
      - `dec:sourceArtifact` = tc_iri (this TC), AND
      - `dec:sourceSession` traces back to graph_iri (this
        graph's prior VGR — match by VGR IRI prefix or by
        following `dec:resultOf` edges), AND
      - `dec:lifecycleState` is in `{produced, routed,
        received}` (not already terminal).
   c. For each match, write the lifecycle transition:
      - `<fb> dec:lifecycleState "closed"`
      - `<fb> dec:closedByEvidenceRetraction <VGR-N>`
      - `<fb> dec:closedAt <now>`
      - `<fb> dec:closedReason "evidence retracted by approved
        VGR-N"`
3. **Commit transaction.** VGR-N and the lifecycle transitions
   land atomically. A reader that sees VGR-N also sees the
   closed defects; there's no observable interleaving.
4. **Failure handling.** If the auto-close transitions fail
   SHACL validation (shouldn't happen — they're routine
   lifecycle edits — but defence in depth), the VGR-N commit
   itself rolls back. The verifier reports a write-error to
   the harness; the dispatch fails. Operator inspects the
   SHACL violation; this is a real bug to fix.
5. **Diagnostic CLI:** `dec _retract-stale-defects --graph
   VG-NNN` reads the graph's most-recent approved VGR and
   runs steps 2–3 against it. `--dry-run` lists what would
   close without writing. Operator-driven backfill path for
   stores predating FT-116.

### Invariants

- Defects with `outcome = "fail"` in the new VGR are NOT
  closed. Only TCs whose evidence flipped from fail→pass get
  their prior defects retracted.
- Defects from a different graph are NOT touched. Even if the
  new VGR's TC matches an open defect on graph G', G'
  hasn't been re-verified by VGR-N. (Distinct evidence
  streams stay independent.)
- Defects already in terminal state (`closed`, `rejected`,
  `superseded`) are NOT modified. The auto-close is a
  one-shot transition out of the live states only.
- The auto-close transition is monotonic: once closed, never
  reopened. If a later VGR fails the same TC, that's a NEW
  defect with a new IRI, not a reopen of the closed one.
- The VGR-N commit and the lifecycle transitions are
  atomic — either both land or neither does. No observable
  half-state where VGR exists but defects are still open
  (or worse, closed without the supporting VGR).
- `dec loop show FT-XXX` after auto-close shows previously-
  open defects as `[closed]` with the citation `closed by
  VGR-N (evidence retraction)`.

### Error handling

- Auto-close pass finds zero candidates: no-op, no error.
- SHACL validation rejects a transition: roll back the entire
  VGR-write transaction, report the error. This is the loud
  failure mode because it indicates a vocab / shape bug.
- Source-session IRI can't be resolved to a graph (the
  feedback was emitted by a non-VGR session — e.g. a worker
  that emitted defect feedback outside of a verify step): the
  candidate is skipped, not closed. We only auto-close when
  the chain of evidence is unambiguous.
- The diagnostic CLI invoked with `--graph VG-XXX` against a
  graph whose latest VGR is `rejected` (not approved):
  no-op + message "no approved VGR to retract from."

### Boundaries

- This feature does NOT introduce new "evidence retraction"
  semantics beyond the auto-close transition. Cross-graph
  evidence reconciliation (e.g. "TC-X passed in VGR from
  graph A and failed in VGR from graph B — which wins?") is
  a separate concern.
- This feature does NOT change how defects get emitted by
  failing VGRs. The defect-emission path is unchanged; only
  the close path gains the auto-mechanism.
- This feature does NOT modify `dec:addressedBy` semantics.
  Worker-driven closure via `addressed_feedback_iris`
  continues to work as today and remains the primary path
  for defects that require worker action. Auto-close is the
  secondary path for the narrower "evidence retracted" case.
- This feature does NOT auto-close defects emitted by the
  implementer or VGA workers (those have different
  source-session shapes). Only verifier-emitted defects
  (i.e. those whose source-session is a VGR-NNN
  verify-feature activity) get the treatment.

## Out of scope

- **Reopening closed defects when a subsequent VGR shows the
  TC failing again.** The lifecycle state machine is
  intentionally monotonic per ADR-024; re-emitting on
  re-failure produces a fresh defect IRI rather than
  reopening a closed one. Mixing reopens into the SM is a
  bigger semantic change than this feature warrants.
- **Cross-graph evidence reconciliation.** Out of scope as
  noted above; future work.
- **Auto-closing across the entire repo at bootstrap.** The
  diagnostic CLI exists for graph-by-graph backfill, but a
  bulk pass over every graph in the store at one moment is
  not provided. Operators can `for VG in $(dec verify graph
  list); do dec _retract-stale-defects --graph $VG; done` if
  they want a sweep.
- **Operator-visible side-channel notifications.** No
  separate CLI message announcing each auto-close — the
  closure shows up in `dec loop show` like any other
  lifecycle transition. Auto-close is bookkeeping, not an
  event the operator needs to react to.
- **Changes to the FT-110 cycle detector or the pairwise
  no-progress detector.** Both detectors continue to read
  `open_defect_feedback_count` from the inspector; the
  inspector continues to compute it via the existing SPARQL.
  FT-116 only changes the underlying lifecycle states, not
  the detectors that read them.
