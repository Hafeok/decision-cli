---
id: FT-097
title: 'decision-cli: VerificationStepTrace + VerificationGraphResult artifact types and multi-graph aggregation'
phase: 3
status: complete
depends-on: []
adrs:
- ADR-028
tests:
- TC-151
- TC-152
- TC-153
domains: []
domains-acknowledged: {}
---

## Description

Two new typed artifacts plus one composition rule. Together they give the slice-3 graph executor ([FT-098](FT-098)) a place to land its per-step trace, give the CLI ([FT-099](FT-099)) something to render, and give the chain-integrity pipeline an aggregate verdict per feature.

- `dec:VerificationStepTrace` — one per step run: outcome (`pass` / `fail` / `unrunnable`), start/end timestamps, captured stdout/stderr fragments (size-capped), and a PROV-O `prov:wasGeneratedBy` link to the run activity. Mirrors the shape of TC runner outputs (ADR-013, two-tier exit) so feature-level verify and graph-level run share one mental model.
- `dec:VerificationGraphResult` — one per `VerificationGraph` execution: holds the ordered list of step traces, the resolved per-step `dec:providesEvidenceFor` projection (which TCs got `pass` / `fail` / `unrunnable` evidence from this run), and a single per-graph `verdict` field consistent with [ADR-018](ADR-018)'s `VerificationVerdict` vocabulary (`approved` / `rejected` / `amendment-required`).
- A pure-function **aggregation rule** that composes one or more `VerificationGraphResult`s for the same target (a `TC`, or a `Feature` rolling its TCs up) into a single `VerificationVerdict`. The rule is the one stated in [ADR-028](ADR-028) §Multi-graph aggregation, lifted from prose into a typed function with explicit tie-breaking.

This is the **artifact-types slice** — no executor, no CLI, no subscription. [FT-098](FT-098) writes these artifacts; [FT-099](FT-099) reads them; [FT-100](FT-100) fires the writers. Splitting the typed result from the executor keeps the SHACL surface stable while the executor's internals are still in flux.

One subcommand → one slice — there is no subcommand here; the slice covers two artifact types + one aggregation function and its tests. Persistence routing lives in [FT-098](FT-098) (the runner is the only writer), but the **typed shapes are owned here** so other consumers (fitness functions, dashboards, replay) can read them without depending on the runner crate.

## Functional Specification

### Inputs

This feature defines artifact shapes and a pure aggregation function; its only "input" is the existing graph store + the runner's calls. Concretely:

- The aggregation function takes a `Vec<VerificationGraphResult>` and a target (`TC` or `Feature` IRI) and returns a `VerificationVerdict` value (not a persisted artifact — the verdict is returned to the caller, who decides whether to persist it).
- The Rust type lives in `crates/decision-cli/src/core/ontology/verification_result.rs` (next to the existing `verification_environment.rs` and `verification_graph.rs`) so every feature slice that needs the type imports from `core::ontology`.

### Outputs

- `.dec/verify/result/VGR-NNN.ttl` — one Turtle file per executed graph, written by the runner ([FT-098](FT-098)). Named with a stable `VGR-` prefix mirroring `VG-`, `ENV-`, `CW-` conventions. **Authoritative on disk** (per [ADR-028](ADR-028) §Storage format); the in-store projection is rebuilt from the file.
- The Rust type `core::ontology::VerificationGraphResult` (and its companion `VerificationStepTrace`) — `serde`-serialisable, with `from_turtle`/`to_turtle` helpers reusing the existing `core::graph::turtle` plumbing.
- No CLI output from this feature directly.

### State

- New on-disk directory `.dec/verify/result/` (created lazily by [FT-098](FT-098); this slice only declares the shape).
- New ontology types projected into the `dec:` namespace via the embedded ontology bundle (extends [FT-006](FT-006)).
- New SHACL shapes (`dec:VerificationStepTraceShape`, `dec:VerificationGraphResultShape`) shipped through the same bundle path as [FT-036](FT-036).

### Behaviour

#### Turtle shape — `VerificationStepTrace`

```turtle
<https://decision-cli.dev/ns/result/VGR-001/step/0>
    a dec:VerificationStepTrace ;
    dec:tracesStep      <https://decision-cli.dev/ns/step/VG-001/0> ;
    dec:outcome         "pass" ;            # pass | fail | unrunnable
    dec:startedAt       "2026-05-26T14:00:00Z"^^xsd:dateTime ;
    dec:endedAt         "2026-05-26T14:00:00.420Z"^^xsd:dateTime ;
    dec:exitCode        0 ;                 # nullable for non-shell steps
    dec:stdoutExcerpt   "ok\n" ;            # cap at 4 KiB; longer payloads truncated with marker
    dec:stderrExcerpt   "" ;
    dec:errorMessage    "" ;                # populated when outcome != pass
    prov:wasGeneratedBy <activity/run/RUN-NNN> .
```

Fields:

| Field | Type | Required | Notes |
|---|---|---|---|
| `dec:tracesStep` | IRI of a `dec:VerificationStep` | yes | The step in the parent VG this trace records. |
| `dec:outcome` | `"pass"` / `"fail"` / `"unrunnable"` | yes | Two-tier outcome per [ADR-013](ADR-013); `unrunnable` covers timeouts, missing targets, op-violations caught at run time. |
| `dec:startedAt`, `dec:endedAt` | xsd:dateTime | yes | UTC, ISO 8601. |
| `dec:exitCode` | xsd:integer | no | Populated for `shell-command` and `http-request`; absent for `capture`. |
| `dec:stdoutExcerpt`, `dec:stderrExcerpt` | xsd:string | yes | Capped at 4 KiB each; truncation marker `"…[truncated N bytes]"`. |
| `dec:errorMessage` | xsd:string | no | One-line summary when `outcome != pass` (e.g. `"expected exit 0, got 1"`, `"sparql returned 3 rows, expected 1"`). |
| `prov:wasGeneratedBy` | IRI of the run activity | yes | Closes the PROV-O chain ([ADR-004](ADR-004)). |

#### Turtle shape — `VerificationGraphResult`

```turtle
<https://decision-cli.dev/ns/result/VGR-001>
    a dec:VerificationGraphResult ;
    dec:resultOf        <https://decision-cli.dev/ns/graph/VG-001> ;
    dec:ranInEnvironment <https://decision-cli.dev/ns/env/ENV-001> ;
    dec:verdict         "rejected" ;        # approved | rejected | amendment-required
    dec:startedAt       "2026-05-26T14:00:00Z"^^xsd:dateTime ;
    dec:endedAt         "2026-05-26T14:00:01.130Z"^^xsd:dateTime ;
    dec:stepTraces      ( <result/VGR-001/step/0> <result/VGR-001/step/1> <result/VGR-001/step/2> ) ;
    dec:evidenceFor     [
        a dec:EvidenceProjection ;
        dec:tc          <https://decision-cli.dev/ns/tc/TC-144> ;
        dec:outcome     "fail" ;
        dec:fromStep    <result/VGR-001/step/2>
    ] ;
    dec:rationale       "step 2 (sparql-assertion) returned 0 rows; expected 1" ;
    prov:wasGeneratedBy <activity/run/RUN-NNN> ;
    prov:wasAttributedTo <agent/runner> ;
    dcterms:created     "2026-05-26T14:00:01.130Z"^^xsd:dateTime .
```

Fields:

| Field | Type | Required | Notes |
|---|---|---|---|
| `dec:resultOf` | IRI of `dec:VerificationGraph` | yes | The graph this run executed. |
| `dec:ranInEnvironment` | IRI of `dec:VerificationEnvironment` | yes | Echoed from the graph; recorded explicitly so a result is self-contained for replay. |
| `dec:verdict` | `"approved"` / `"rejected"` / `"amendment-required"` | yes | Per-graph verdict from the *single-graph* derivation below. |
| `dec:startedAt`, `dec:endedAt` | xsd:dateTime | yes | Wall-clock envelope of the full run. |
| `dec:stepTraces` | `rdf:List` of `VerificationStepTrace` IRIs | yes | Ordered, matches the parent VG's `dec:steps` ordering. |
| `dec:evidenceFor` | unordered set of `EvidenceProjection` blank nodes | yes | One projection per `(TC, step)` pair where a step declared `dec:providesEvidenceFor`. Empty list is allowed (graph has no TC-linked steps). |
| `dec:rationale` | xsd:string | yes | One-line human summary, ≥ 20 chars (matches [ADR-018](ADR-018)'s rationale minLength). |
| `prov:wasGeneratedBy` | IRI | yes | Run activity; ties result to the dispatch session per [FT-069](FT-069). |
| `prov:wasAttributedTo` | IRI | yes | The runner role agent. |
| `dcterms:created` | xsd:dateTime | yes | Persistence time. |

`EvidenceProjection` is a **derived** blank-node structure — it's a materialised join between the parent graph's `dec:providesEvidenceFor` annotations and this run's per-step outcomes. The runner is the only writer; downstream readers (CLI, fitness functions) consume it as queryable data without re-deriving.

#### Per-graph verdict derivation (single-graph rule)

The runner sets `dec:verdict` on each `VerificationGraphResult` according to the per-step outcomes:

- All steps `outcome = "pass"` → `approved`.
- Any step `outcome = "fail"` AND `dec:providesEvidenceFor` is non-empty on that step → `rejected`.
- Any step `outcome = "fail"` AND `dec:providesEvidenceFor` is empty (a setup or capture step failing) → `amendment-required` (the procedure broke before it could produce evidence; the graph needs editing, not the code).
- Any step `outcome = "unrunnable"` AND no other step is `fail` → `amendment-required`.
- All other combinations: `rejected`.

The rationale string is populated by the runner ([FT-098](FT-098)) to name the dominant cause; this slice only specifies that the field is required and ≥ 20 chars.

#### Multi-graph aggregation rule (the composition function)

```rust
pub fn aggregate_verdict(
    target: AggregationTarget,         // TC IRI or Feature IRI
    results: &[VerificationGraphResult],
) -> AggregateVerdict;

pub struct AggregateVerdict {
    pub verdict: Verdict,              // approved | rejected | amendment-required
    pub rationale: String,
    pub contributing_results: Vec<Iri>,// the VGR IRIs that drove this verdict
    pub coverage_gaps: Vec<Iri>,       // TC IRIs that have *no* contributing result
}
```

The verdict is derived from the filtered set `R = { r ∈ results : r covers target }`:

- A `VerificationGraphResult r` *covers* a `TC` iff some `EvidenceProjection` in `r.evidenceFor` has `dec:tc = TC`.
- A `VerificationGraphResult r` *covers* a `Feature F` iff it covers at least one `TC ∈ F.tests`.

Aggregation rule (matches [ADR-028](ADR-028) §Multi-graph aggregation, with tie-breaking made explicit):

| Set membership | Aggregate verdict |
|---|---|
| `R` is empty | `rejected`, with rationale `"no verification graph result covers <target>"`. The `coverage_gaps` field carries the uncovered TCs. |
| All `r ∈ R` have `verdict = approved` | `approved`. |
| Any `r ∈ R` has `verdict = rejected` | `rejected`. (Rejection dominates — one failed verification ends the matter regardless of other passes.) |
| Otherwise (mix of `approved` and `amendment-required`, no `rejected`) | `amendment-required`. |

For a `Feature` target, the rule applies **per-TC** and then composes: the feature's aggregate verdict is the worst per-TC verdict (`rejected` > `amendment-required` > `approved`). A TC with no covering result contributes `rejected` per the empty-set row. The `coverage_gaps` field reports which TCs lack any covering result so the operator can run the missing graphs.

The function is **pure** — no IO, no graph access. Callers pass in the materialised `VerificationGraphResult` set (typically loaded via a SPARQL query the function ships alongside).

### Invariants

- `VerificationGraphResult` is **immutable once written**. A re-run produces a *new* `VGR-NNN`, not an update. Lineage is via `prov:wasGeneratedBy` to a new activity. This preserves replay and audit; the chain-integrity gate ([ADR-031](ADR-031)) consumes the latest-by-`dcterms:created` per `(graph, env)` tuple.
- `dec:stepTraces` ordering **must** match the parent `VerificationGraph.dec:steps` ordering element-for-element (same length, same step IRIs in the same position). SHACL enforces length parity; ordering parity is asserted by a SPARQL constraint in the shape file. A violation indicates a runner bug, not a content bug.
- Every step IRI referenced by `dec:tracesStep` **must** exist in the parent graph at write time. SHACL `sh:in` over the dereferenced step set. Detected at `StreamWriter` commit ([FT-001](FT-001), [FT-073](FT-073)).
- `dec:verdict` on a `VerificationGraphResult` is derived from `dec:stepTraces`; the SHACL shape carries a `sh:sparql` constraint that re-asserts the per-graph verdict rule above, so a result whose verdict doesn't match its trace pattern is rejected at write time. This is the equivalent of the per-TC SHACL on `VerificationVerdict` ([ADR-018](ADR-018)).
- The aggregation function is **deterministic** — same `results` input, same target, same output. Tested with property-based tests over generated `VerificationGraphResult` vectors.
- Excerpt fields (`stdoutExcerpt`, `stderrExcerpt`) are **size-capped at 4 KiB** at the writer. Full payloads, if needed, live in a sibling `.dec/verify/result/VGR-NNN/step-N.{stdout,stderr}.log` file referenced by `dec:stdoutFullRef` / `dec:stderrFullRef` (optional predicates; out of scope for this slice — declared in the shape, populated by a later feature).

### Error handling

- Writing a `VerificationGraphResult` whose `stepTraces` length differs from the parent graph's step count → `Error::SchemaViolation` from `StreamWriter`; refuses to persist.
- Writing a `VerificationStepTrace` referencing a step IRI not in the parent graph → `Error::SchemaViolation`.
- Aggregation function called with `results` from different targets (e.g. a result for `VG-A` mixed with one for `VG-B` against the same TC) is **valid** — both contribute to the per-TC verdict. The function does not de-duplicate; the caller decides what to feed in.
- Aggregation function called with an empty `results` vector for a target that has no listed TCs → returns `approved` with rationale `"target has no TCs; vacuous pass"`. This is the documented degenerate case; the caller can choose to escalate it to a gap.

### Boundaries

- **In scope.** The two artifact types (Rust struct + Turtle shape + SHACL shape), the embedded-ontology delta, the file-naming convention (`VGR-NNN.ttl`), the per-graph verdict derivation rule, the pure-function aggregation API, and unit + property tests for the aggregation rule. SHACL shapes ship in the same bundle path as [FT-036](FT-036), validated at `StreamWriter` commit time.
- **Out of scope.** The runner that *produces* these artifacts ([FT-098](FT-098)). The CLI that *displays* them ([FT-099](FT-099)). The subscription that *fires* the runner ([FT-100](FT-100)). The `--platform` / fitness-function side of [ADR-014](ADR-014) consuming aggregate verdicts (later slice). The per-step full-log files (`dec:stdoutFullRef` etc. are reserved predicates but unpopulated). Cross-environment de-duplication policy (the aggregation function takes whatever the caller hands it).

## Out of scope

- Executor implementation.
- CLI rendering of results.
- Auto-dispatch wiring.
- Full-payload log-file persistence (excerpt only this slice).
- Verdict-driven feedback emission (lives in [FT-098](FT-098), the runner).
- Updating the chain-integrity gate ([ADR-031](ADR-031) / [FT-047](FT-047)) to consume aggregate verdicts (separate slice once the runner has produced its first results).
