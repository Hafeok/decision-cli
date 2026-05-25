---
id: FT-074
title: 'decision-cli: Migrate existing Feature/ADR/TC/Dependency artifacts to dual-provenance conformance (grandfather + backfill)'
phase: 3
status: complete
depends-on:
- FT-073
adrs:
- ADR-042
- ADR-013
- ADR-016
tests:
- TC-124
domains:
- data-model
- error-handling
domains-acknowledged:
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-054: ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  ADR-041: ADR-041 governs SHACL enforcement at the GraphWriter chokepoint. This feature does not write artifacts through GraphWriter.
  ADR-040: ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact.
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-047: ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-039: ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates.
  ADR-044: ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-038: ADR-038 governs dual-provenance discipline (mechanical + motivational). This feature does not introduce a new artifact type subject to dual provenance.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
---

## Description

Migrate product-cli's existing Feature, ADR, TC, and Dependency artifacts to dual-provenance conformance (ADR-038) using the grandfather-with-backfill policy (ADR-042). The migration runs once; produces synthetic mechanical-provenance triples for backfillable artifacts; flags genuine orphans as Feedback for human repair; runs in warn-only mode against the FT-073 validator until orphan count drops below the operator-configured threshold; then GraphWriter cutover flips validation to reject mode.

Migration tooling is itself an artifact-producing path that conforms to the discipline — synthetic `:HistoricalSession` and `:HistoricalAgent` artifacts produced by the migration carry `MigrationBackfill` boundary-artifact class membership per FT-071. Recursion holds.

## Functional Specification

### Inputs

- The existing artifact corpus under `.product/` (Features, ADRs, TCs, Dependencies) plus any other artifacts the orchestration store has accumulated.
- The dual-provenance shape set (FT-072) for evaluating conformance.
- A mapping from informal front-matter fields to motivational predicates (encoded in the migration tool; see Behaviour below).
- `dec migrate provenance --dry-run` and `dec migrate provenance --apply` operator commands.

### Outputs

- `crates/decision-cli/src/features/ft_074_migrate_provenance/` — slice directory containing:
  - `audit.rs` — the three-class audit (conformant / backfillable / orphan).
  - `backfill.rs` — synthetic mechanical-triple production with `:HistoricalSession` / `:HistoricalAgent` / `:isMigrationBackfill` annotation.
  - `orphan_feedback.rs` — Feedback emission for unrepairable cases.
  - `mapping.rs` — front-matter-to-motivational mapping table.
  - `commands.rs` — CLI handler for `dec migrate provenance`.
- New CLI subcommand `dec migrate provenance [--dry-run|--apply] [--cutover-threshold N]`.
- A migration report file at `.product/.migrations/provenance-<timestamp>.json` with per-artifact verdicts.
- Feedback artifacts of class `migration-orphan-needs-repair` for every orphan, routed to the operator-curator role per FT-029.
- A new operator command `dec migrate provenance cutover` that flips GraphWriter (FT-073) from warn-only to reject mode after orphan count is below threshold.

### State

- During migration window: GraphWriter's validator (FT-073) runs in *warn-only* mode. Violations are logged + emitted as Feedback but do not reject writes.
- After cutover: warn-only flag is removed; validation rejects on every write.
- Migration report files accumulate under `.product/.migrations/` as an audit trail.
- The migration is idempotent: re-running it on an already-migrated corpus produces no new backfills but re-emits Feedback for any orphan that has not been repaired.

### Behaviour

1. **Audit pass** — classify every artifact in the corpus:

   ```rust
   enum AuditVerdict {
       Conformant,                                          // both blocks present already
       BackfillableMechanical { mapping: Vec<EdgeMap> },    // motivational present via informal fields; mechanical missing
       Orphan { reasons: Vec<&'static str> },               // neither present, no informal edges to map
   }
   ```

   The audit runs a SHACL validation pass over each artifact, classifying by which constraints fail.

2. **Front-matter to motivational mapping** — the slice-1 mapping table:

   | Source field | Source type | Motivational predicate | Target type | Notes |
   |---|---|---|---|---|
   | Feature.`adrs` | Feature | (no edge produced) | — | The reverse direction is what matters; produces ADR `:decides_for` edges below |
   | ADR.`features` | ADR | `:decides_for` | Feature | |
   | TC.`validates.features` | TC | `:validates` | Feature | |
   | TC.`validates.adrs` | TC | `:validates` | ADR | |
   | Dependency.`features` | Dependency | `:required_by` | Feature | (inverted direction) |
   | Dependency.`adrs` | Dependency | `:required_by` | ADR | (inverted direction) |
   | Feature.`depends-on` | Feature | (none — this is a horizontal feature-to-feature link, not motivational) | — | Preserved as a separate `:depends_on` predicate, not motivational |
   | ADR.`supersedes` | ADR | `:supersedes` | ADR | |

   Features whose existing `adrs:` list is the only signal of motivational origin remain *orphans* in the new vocabulary because the Feature→ADR direction isn't motivational (the ADR motivates the Feature, not the other way). The migration emits guidance Feedback explaining the gap; manual repair adds the missing `addresses` / `decomposes_from` / `originated_from` / `responds_to` edge.

3. **Mechanical backfill** — for backfillable artifacts (those with a motivational mapping but no mechanical block):

   - Create one `:HistoricalSession` artifact per migrated artifact (slice-1 choice: per-artifact for traceability; sharing a session across a batch is a slice-2+ optimisation if storage growth becomes a concern).
   - Create one shared `:HistoricalAgent` (`dec:agent:historical-pre-discipline`) attributed to "pre-discipline authorship by the operator." This Agent is itself a BoundaryArtifact of subclass `BootstrapArtifact`.
   - Assert on the migrated artifact:
     - `prov:wasGeneratedBy <historical-session-iri>`
     - `prov:wasAttributedTo dec:agent:historical-pre-discipline`
     - `prov:generatedAtTime <git-first-commit-timestamp>` (from `git log --follow --format=%aI` on the artifact file, or the migration run timestamp if git history is unavailable).
   - Mark the synthetic session: `:isMigrationBackfill true`, `:migrationNote "<reason>"`.

4. **Orphan flagging** — for orphans, emit a Feedback artifact:

   ```rust
   Feedback {
       class: "migration-orphan-needs-repair",
       artifact_ref: orphan_iri,
       reasons: vec!["No mechanical block", "No motivational mapping"],
       suggested_repair: ".. type-specific guidance from FT-070's table ..",
   }
   ```

   The orphan itself is annotated `:isMigrationOrphan true` so writes that touch it can emit a warning, but the artifact remains readable.

5. **Bootstrap subcase** — artifacts written before any Session existed (the FT-009 catalog bootstrap, the FT-006 ontology seed, etc.) are migrated under a single `:BootstrapSession` IRI attributed to `:BootstrapAgent`. Both are themselves `BoundaryArtifact` instances of subclass `BootstrapArtifact`. This resolves Brief open question 6.

6. **Cutover** — `dec migrate provenance cutover` checks the current orphan count. If below the configured threshold (default 0; operator can override), it removes the warn-only flag from GraphWriter's validator config, persisting the new mode to the store. Validation now rejects on every write.

7. **Idempotence** — re-running migration after partial repair re-evaluates each artifact. Already-conformant artifacts are skipped. Already-backfilled artifacts are skipped (the `:isMigrationBackfill` annotation is the marker). New orphans (e.g. an artifact added during the window without conforming) are flagged.

### Invariants

- **Synthetic triples are always queryable as synthetic.** `:HistoricalSession` and `:HistoricalAgent` carry `:isMigrationBackfill true`; backfilled `wasGeneratedBy` edges point at sessions carrying that flag. Queries that need real provenance filter via `FILTER NOT EXISTS { ?session :isMigrationBackfill true }`.
- **The migration produces no false motivational edges.** Only edges that have a clear informal-field source are emitted. When in doubt, classify as orphan rather than backfill.
- **Cutover is operator-triggered.** Migration tooling reports orphan count; it does not auto-cut over. ADR-042 specifies the policy; the operator picks the moment.
- **Migration is reversible.** Backfill-produced triples can be removed by a single SPARQL DELETE filtered on `:isMigrationBackfill true`. The migration report file enumerates every triple inserted, so an explicit rollback path is also available.
- **The migration tool conforms to the discipline.** Synthetic sessions and agents themselves carry mechanical provenance (the migration session that produced them — itself a `:HistoricalSession` of class `BootstrapArtifact`) and external_origin (`"FT-074 provenance migration tool run at <timestamp>"`). Recursion terminates at the `BoundaryArtifact` boundary.

### Error handling

- A backfillable artifact whose synthetic-session creation fails mid-batch → migration aborts with the partial-state report. Re-run after repair; idempotence ensures only the unfinished artifacts retry.
- An orphan that cannot have its Feedback emitted (Feedback shape itself violated, somehow) → migration logs and proceeds; the orphan is still annotated `:isMigrationOrphan true` so it remains visible.
- Cutover requested while orphan count > threshold → command exits 1 with the count and the list of unrepaired orphans.
- Git timestamp lookup failure (artifact file untracked or git unavailable) → use the migration run timestamp; log a per-artifact warning.

### Boundaries

- **In scope.** The audit pass. The backfill producer. The orphan Feedback emitter. The mapping table. The `dec migrate provenance` CLI surface (dry-run, apply, cutover). The migration report file. The `:BootstrapSession` synthetic for pre-Session-era artifacts. Idempotence and reversibility.
- **Out of scope.** Continuous orphan-detection fitness function (slice 2+ per Brief excludes — that's a steady-state check, not migration). Retroactive motivational inference for orphans (slice 3+ per Brief excludes). Cross-system migration; this is single-graph only.

## Out of scope

- Automated repair of orphans. Slice 1 emits Feedback; humans repair.
- Migration of artifacts in another system's graph. Each system migrates its own corpus.
- Lossy schema transformations (e.g. collapsing two motivational predicates into one). Not part of slice 1.

## References

- [ADR-042](ADR-042) — Migration policy (the decision this feature implements).
- [ADR-038](ADR-038) — Dual-provenance discipline (the target invariant).
- [ADR-040](ADR-040) — BoundaryArtifact (the class synthetic sessions and agents inhabit).
- [FT-071](FT-071) — BoundaryArtifact + MigrationBackfill subclass + shape (the validator constraint this feature must satisfy on its synthetic outputs).
- [FT-073](FT-073) — Validator (runs in warn-only mode during migration window; flipped at cutover).
- [FT-026](FT-026), [FT-029](FT-029) — Feedback artifact + routing for orphan emissions.
