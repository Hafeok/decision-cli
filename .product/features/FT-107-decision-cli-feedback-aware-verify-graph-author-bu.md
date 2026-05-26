---
id: FT-107
title: 'decision-cli: Feedback-aware verify-graph-author bundles and matcher bypass for re-authoring'
phase: 3
status: complete
depends-on:
- FT-049
- FT-099
- FT-102
adrs:
- ADR-026
- ADR-030
- ADR-066
tests:
- TC-183
- TC-184
- TC-185
- TC-186
domains: []
domains-acknowledged: {}
---

## Description

Close the feedback loop for the verify-graph-author role so existing broken graphs can be re-authored from runtime evidence.

Today, when a `VerificationGraph` runs and emits `dec:Feedback` with `class = "defect"` targeting the `verifier` role (per [ADR-026](ADR-026) routing), the feedback lands in the store but the verify-graph-author has no way to consume it. Two structural reasons:

1. **The bundle assembler doesn't include feedback.** `VerifyGraphAuthorInputJson` (per [FT-102](FT-102)) carries the feature_spec, TCs, env, candidate graphs, and the five [ADR-066](ADR-066) enrichment fields — but no field for the runtime evidence that the existing covering graph is broken.
2. **The matcher short-circuits on TC coverage alone.** `verify_graph_generate::run_generate` calls `MatchKind::CompleteSingle | CompleteMultiple` → `build_match_response`, which returns "VG-N already covers this feature in this environment; no new graph needed". Coverage is measured by TC-linkage in the catalog; verdict quality is invisible to the matcher.

Together these block re-authoring even when the operator can point at concrete defect feedback explaining what failed. The 432 `produced` defect-feedback artifacts in the current store are dead letters until this closes.

One subcommand → one slice — there is no new subcommand. The slice modifies the existing `dec verify graph generate` dispatch handler, the bundle envelope, the matcher's gating rule, and the worker prompt. The change is bigger than a one-liner but narrower than two coupled deliverables would justify.

## Functional Specification

### Inputs

#### 1. The bundle assembler (orchestrator side)

`features/verify_graph_generate/bundle.rs::VerifyGraphAuthorInputJson` gains one field:

```rust
pub struct VerifyGraphAuthorInputJson {
    // existing fields per FT-048 / FT-102 ...
    pub defect_feedback: Vec<DefectFeedbackRecord>,
}

pub struct DefectFeedbackRecord {
    pub feedback_iri:    String,  // urn:dec:feedback:<uuid>
    pub class:           String,  // "defect"
    pub severity:        String,  // "error" | "warning"
    pub evidence:        String,  // free-text excerpt the runner wrote
    pub addressing_step: Option<String>,  // step IRI the feedback was emitted for
    pub graph_id:        String,  // VG-NNN the failing step belonged to
    pub result_id:       String,  // VGR-NNN that produced the feedback
    pub emitted_at:      String,  // RFC3339
}
```

Populated by one SPARQL `SELECT` against the orchestration store, filtered to `(feature, env)`:

```sparql
SELECT ?fb ?class ?severity ?evidence ?step ?graph ?result ?emitted WHERE {
  ?fb a dec:Feedback ;
      dec:class           "defect" ;
      dec:targetRole      "verifier" ;
      dec:lifecycleState  "produced" ;
      dec:severity        ?severity ;
      dec:evidenceExcerpt ?evidence ;
      dec:emittedAt       ?emitted ;
      dec:addressingArtifact ?step .
  ?graph dec:verifies <feature_iri> .
  ?graph dec:steps/rdf:rest*/rdf:first ?step .
  ?graph dec:environment <env_iri> .
  ?result dec:resultOf ?graph .
  BIND("defect" AS ?class)
}
ORDER BY DESC(?emitted)
LIMIT 50
```

50 most recent is the cap — the worker doesn't need the full history; the most recent runs carry the salient signal. Bundle hash recomputes over the enriched payload.

#### 2. The matcher gating rule

`verify_graph_generate::run_generate` keeps its match-first dispatch, but the short-circuit gains one condition:

```rust
if matches!(report.kind, CompleteSingle | CompleteMultiple)
    && defect_feedback_for(workdir, &req.feature_id, env_iri).is_empty()
{
    return Ok(build_match_response(&report));
}
```

If the matcher reports complete coverage **and** no actionable defect feedback exists for the pair, short-circuit as today. If defect feedback exists, fall through into worker dispatch — the existing graph is "covering but broken", and the operator wants a re-authored proposal.

The matcher's view of `CompleteSingle`/`CompleteMultiple` stays as-is. This slice does not redefine coverage; it adds a single orthogonal condition to the dispatch decision.

#### 3. The worker (verify-graph-author)

The Python `VerifyGraphAuthorInput` pydantic model extends with the matching `defect_feedback: list[DefectFeedbackRecord]` field. The system prompt gains one section:

> *"If `defect_feedback` is non-empty, an existing graph already covers this feature in this environment but produced these defects at runtime. Read each entry's `evidence` field and the linked step's command/path to understand what failed. Your proposal should fix the underlying problem — most often a mismatch between the env type (`ephemeral-tempdir` vs `repo-path`) and the commands the steps run. Cite the feedback IRIs you addressed in your rationale."*

The worker package update is ~30 lines (model field + one prompt section). The heavy lift is the bundle assembler's SPARQL query and the matcher gate.

### Outputs

- A re-authored `dec:VerificationGraph` proposal that explicitly references the defect feedback in its `rationale`. Proposal kind stays `New`; the existing broken graph is **not** superseded by this slice (supersession is a follow-up — for now both graphs coexist and the operator picks).
- The dispatched session links to every consumed feedback via `dec:respondsToFeedback` (a new predicate on the dispatch activity) so the audit trail shows which defects each proposal addressed.
- When the slice accepts (CLI `--accept` or MCP `dec_verify_graph_accept`), each consumed feedback transitions `produced → addressed` with the new graph as the addressing artifact ([ADR-024](ADR-024) lifecycle).

### State

- No on-disk schema change. The feedback table already exists (`.dec/feedback/`); the dispatch session already exists. The lifecycle transition uses the existing `dec feedback close` writer.
- Reads: feedback artifacts, candidate graphs, existing bundle inputs. No new write paths.

### Behaviour

1. Operator runs `dec verify graph generate FT-XXX --environment ENV-YYY` (or `--accept`).
2. The handler runs the matcher (unchanged).
3. **New:** the handler runs `defect_feedback_for(feature, env)` against the store.
4. If `CompleteSingle | CompleteMultiple` AND `defect_feedback.is_empty()` → short-circuit with the match response (today's behaviour).
5. Otherwise, the handler assembles the bundle (now including the defect feedback) and dispatches the worker.
6. The worker returns a proposal; the existing FT-102 validator runs; on success the proposal is persisted via the existing path.
7. **New:** on accept, each feedback consumed by the proposal transitions to `addressed` with the new graph as the addressing artifact.

### Error handling

- `defect_feedback_for` query failure → log a warning, treat as empty, continue with today's behaviour. The slice degrades to a no-op rather than failing the dispatch.
- Worker proposal returns `kind = Match` despite the defect-driven re-dispatch → reject with `Error::WorkerIgnoredFeedback` and emit one `class = "defect"` feedback against the worker role itself (meta-feedback per [ADR-022](ADR-022)). This catches the case where the worker sees the feedback but produces the same broken graph anyway.

### Out of scope

- Supersession of the original broken graph. Both graphs coexist after re-authoring; the operator chooses which the runner targets via standard graph selection. Supersession is a follow-up FT.
- Feedback for the `spec-author` (the 170 `class = "gap"` artifacts in the current store). Those route to a different role and need a different bundle — separate FT.
- Routing-table changes. [ADR-026](ADR-026) routing is unchanged; this slice only adds *consumption* of feedback that the routing already directs to the verifier role.

## Acceptance

1. With defect feedback present for `(FT-008, ENV-001-ephemeral-cli)`, `dec verify graph generate FT-008 --environment ENV-001-ephemeral-cli` invokes the worker (does not short-circuit) and the bundle JSON contains a non-empty `defect_feedback` array.
2. With no defect feedback present for `(FT-XXX, ENV-YYY)`, the matcher short-circuit fires as today (no behaviour change for the green path).
3. On `--accept` of a re-authored proposal, every feedback entry the worker consumed transitions from `produced` to `addressed` with the new graph IRI as the addressing artifact, and the session has the `dec:respondsToFeedback` predicate linking each consumed feedback.
4. A worker that returns `kind = Match` despite non-empty `defect_feedback` in its bundle is rejected with `Error::WorkerIgnoredFeedback`; a meta-feedback artifact is emitted.

## Notes

This slice is the natural follow-up to the bugfix that unblocked the verify pipeline (commit `11e8314`). The runtime now produces high-quality defect feedback; this slice makes that feedback consumable by the role responsible for fixing the graphs.
