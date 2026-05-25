---
id: ADR-042
title: 'Migration policy: grandfather with backfill, quarantine orphans'
status: accepted
features:
- FT-074
supersedes: []
superseded-by: []
domains:
- data-model
scope: domain
content-hash: sha256:b30e9936478989fa338269fd302cc485990288438806c3743311a29d99592a59
---

## Context

product-cli already has Feature, ADR, TC, and Dependency artifacts in production graphs — every artifact in `.product/` is one. The dual-provenance discipline (ADR-038) was not in effect when they were authored; their existing provenance is informal (front-matter `adrs:`, `features:`, `validates:`, `depends-on:` fields that are conceptually motivational but not declared as PROV-O / `wasDerivedFrom` subPredicates, and no mechanical block at all).

Turning SHACL enforcement (ADR-041) on against this corpus would reject every existing artifact. Three migration strategies:

| Option | Behaviour | Tradeoff |
|---|---|---|
| **Reject all non-conformant** | Hard cutover; existing artifacts unreadable. | Strict but breaks production data and stalls everything until manual re-authoring. Untenable. |
| **Grandfather everything** | Existing artifacts exempt forever; discipline applies only to new writes. | No discipline applied retroactively; audit trails for existing artifacts remain informal. The corpus becomes two-tier in perpetuity. |
| **Grandfather with backfill** | Conformant artifacts pass. Artifacts with informal provenance that *maps* to the new vocabulary get backfilled with synthetic mechanical-provenance triples plus explicit migration annotation. Orphans (neither conformant nor mappable) are flagged for human repair. | Preserves production continuity. Applies discipline as fully as the existing data allows. Synthetic triples are marked so consumers that need "real provenance only" can filter them. |

## Decision

**Adopt option 3: grandfather with backfill.**

### Three-class audit

The migration tool classifies every existing artifact in the graph:

1. **Conformant.** Already has both mechanical and motivational blocks. No action needed.
2. **Backfillable mechanical.** Has motivational origin expressible via existing ad-hoc edges (e.g. an ADR's `features:` front-matter list maps to `:decidesFor` motivational edges per FT-070; a TC's `validates:` list maps directly to `:validates` edges) but no PROV-O mechanical block. Migration backfills mechanical triples by attaching:
   - A synthetic `:HistoricalSession` artifact (one per migrated artifact, or one shared per backfill batch — slice-1 chooses per-artifact for traceability) marked `:isMigrationBackfill true`.
   - A synthetic `:HistoricalAgent` representing "pre-discipline authorship" attributed to a generic human-author Agent.
   - A `:migrationNote` literal annotating *why* the backfill was applied (e.g. "Front-matter `adrs:` field interpreted as `:decidesFor` motivational edge under ADR-039 subPropertyOf mapping").
   - `prov:generatedAtTime` set to the artifact file's git first-commit timestamp where available, else the migration run time.
3. **Orphan.** Neither conformant nor mappable. Examples: hand-edited test fixtures, artifacts whose origin was conversational and unrecorded, partially-deleted historical entries. Flagged via a Feedback artifact (ADR-022, class `migration-orphan-needs-repair`) routed to a human-curator role.

### Grandfather rule and quarantine

- **Conformant + backfilled artifacts pass** under the new discipline.
- **Orphans are quarantined** (visible but flagged via a queryable `:isMigrationOrphan true` annotation). Reads continue. Writes that *touch* orphan artifacts (e.g. a new ADR superseding an orphan ADR) emit a warning so authors can repair-while-they're-there.

### Cutover

GraphWriter's SHACL enforcement (ADR-041) is turned on for all writes once the orphan count is below an agreed threshold — set by the operator (slice-1 default: zero unrepaired orphans, with explicit override flag). Until cutover, validation runs in *warn-only* mode: violations are logged and emitted as Feedback but do not reject the write. After cutover, validation rejects.

### Synthetic-triple annotation

Every triple introduced by backfill carries `:isMigrationBackfill true` on the predicating Session/Agent artifact. Queries that need "real provenance only" filter:

```sparql
FILTER NOT EXISTS { ?session :isMigrationBackfill true }
```

The annotation is part of the discipline, not a hack — provenance integrity is preserved *at the meta-level* even where it cannot be reconstructed at the artifact level.

### Bootstrap subcase

Artifacts written before any Session existed (catalog bootstrap, initial shape files, the WorkerCurator role declaration) receive a single synthetic `:BootstrapSession` attributed to a `:BootstrapAgent` representing the human operator who initialized the system. Acceptable because it is one-time; subsequent writes have real Sessions. The Brief's open question 6 calls this out explicitly.

### Alternatives considered

Listed above. Briefly:

- Hard rejection is operationally impossible without a re-authoring sprint nobody scheduled.
- Permanent grandfathering means the audit principle is best-effort forever on the corpus that exists today, which defeats the discipline's purpose.

Grandfather + backfill is the pragmatic middle: discipline applied where it can be reconstructed, quarantine where it cannot, explicit annotation so the synthetic origin of backfilled triples is queryable.

## Consequences

**Positive.**

- Existing corpus migrates without manual re-authoring of conformant + backfillable artifacts (the majority).
- Orphans surface concretely as Feedback for triage rather than rotting silently.
- Synthetic triples are queryable as synthetic, so cost-of-backfill is auditable and reversible if the backfill heuristic turns out to be wrong.

**Negative / accepted costs.**

- Backfill creates synthetic triples that do not represent real historical events. The `:isMigrationBackfill true` annotation mitigates this for queries that care, but consumers that forget the filter will conflate real and synthetic provenance.
- The mapping from front-matter fields to motivational predicates is itself a slice-1 decision and may be wrong in edge cases. The Brief's slice-2+ retroactive-motivational-inference excluded feature exists to revisit; until then, the mapping is fixed at the values chosen in FT-074.
- Cutover criterion ("orphan count below threshold") is a policy call. Slice 1 ships the tooling; the operator decides when to flip.

**Boundary enforcement.** The migration tooling writes through GraphWriter like everything else; backfilled triples are subject to the same validation they enable. The `:HistoricalSession` / `:HistoricalAgent` types are themselves declared as artifact types with their own shapes, so the backfill writes are conformant — recursion holds.

## Relationship to existing ADRs

- **ADR-038 / ADR-040 / ADR-041.** This ADR is the migration policy that lets ADR-041's enforcement turn on without breaking the existing corpus.
- **ADR-022 (Feedback).** Orphan flagging uses the standard Feedback flow.

## Status

Proposed. Implementation in FT-074. Cutover is a separate operational milestone; ADR-042 specifies the policy but does not schedule the cutover.
