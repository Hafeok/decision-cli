---
id: ADR-028
title: Verification as a typed graph in a typed environment
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: domain
content-hash: sha256:3762470f6dd0e5c9edce0fe733d0d3463ad0d9f2bcbed3bf00336735c50d17a9
---

## Context

[ADR-020](ADR-020) decided slice-2's verifier worker would be a single-shot LLM call: the worker receives a bundle (`CodeChange`, `feature_spec`, TCs, ADRs), reasons over it once, returns a `VerificationVerdict`. That decision was right for slice 2 — it proves the action-interpretation loop closes — but it leaves verification informal in two important ways:

1. **The "what to verify" lives in prose.** TCs ([ADR-018](ADR-018)) name success criteria declaratively ("worker exits 0 iff…") but say nothing about *how* to determine the claim holds. The human implementer translates each TC into a procedure each time, and the verifier LLM does the same — re-discovering the procedure on every dispatch. There is no reusable, queryable record of "here is how we prove this feature satisfies its intent."
2. **The "where to verify" is implicit.** A claim like "the artifact reached production" requires an execution context (the production endpoint, read-only credentials, no destructive ops). A claim like "the local build works" requires an ephemeral sandbox. The verifier currently has no first-class way to declare which context it reasons in, and the orchestrator has no way to refuse a dispatch whose context doesn't permit its steps.

These two gaps are connected: verification is itself a graph (procedure) executed against an environment (context). Until both are first-class artifacts, verification is non-composable, non-replayable, and the manual-to-automatic transition is a rewrite rather than a wiring change.

The action-interpretation pattern ([ADR-019](ADR-019), [ADR-017](ADR-017)) reinforces the framing: the implementer produces a `CodeChange` (action); the verifier runs a verification graph against the appropriate environment (interpretation) and produces a verdict. With graphs and environments first-class, the verifier role's input becomes `(VerificationGraph, Environment, CodeChange)` — environment-aware, not environment-blind.

## Decision

Two new artifact types in the dec ontology, both stored as raw Turtle under `.dec/verify/`. A typed step vocabulary with declared `requiredOps`. Safety gating where `step.requiredOps ⊆ env.allowedOps`. Aggregate verdicts across multiple graphs per TC.

### `dec:VerificationEnvironment`

A typed execution context. Stored at `.dec/verify/env/ENV-NNN.ttl`.

```turtle
<env:ephemeral-cli> a dec:VerificationEnvironment ;
    dec:envType         "ephemeral-tempdir" ;
    dec:setup           "mkdir -p $TMPDIR && cd $TMPDIR" ;
    dec:teardown        "rm -rf $TMPDIR" ;
    dec:allowedOps      ( "shell" "filesystem" "sparql-local" ) ;
    dec:safetyClass     "isolated" .

<env:dev-deployment> a dec:VerificationEnvironment ;
    dec:envType         "remote-http" ;
    dec:endpoint        "https://dev.decision-cli.dev" ;
    dec:allowedOps      ( "http" "sparql-http" ) ;
    dec:safetyClass     "shared-non-destructive" .

<env:prod-deployment> a dec:VerificationEnvironment ;
    dec:envType         "remote-http" ;
    dec:endpoint        "https://decision-cli.dev" ;
    dec:allowedOps      ( "http-readonly" ) ;
    dec:safetyClass     "production-readonly" .
```

Properties: `dec:envType`, `dec:setup`, `dec:teardown`, `dec:allowedOps` (rdf:List of operation tokens), `dec:safetyClass`, `dec:endpoint` (optional, for remote types).

Safety classes (controlled vocabulary):

- `isolated` — sandboxed; failure does not affect other systems.
- `shared-non-destructive` — multi-tenant environment; reads and non-mutating writes allowed.
- `production-readonly` — production; only read operations permitted.

The slice-2.5 `dec init` seeds `<env:ephemeral-cli>` so the first verification graph is authorable without a separate setup step.

### `dec:VerificationGraph` containing `dec:VerificationStep`s

A per-feature (or per-TC) procedure pointing at one environment and an ordered list of steps. Stored at `.dec/verify/graph/VG-NNN.ttl`.

```turtle
<vg:FT-001-v1> a dec:VerificationGraph ;
    dec:verifies        <ft:FT-001> ;
    dec:environment     <env:ephemeral-cli> ;
    dec:steps           ( <step:1> <step:2> ) .

<step:1> a dec:VerificationStep ;
    dec:stepType        "shell-command" ;
    dec:command         "dec init --from ./streams/decision-cli-development.ttl" ;
    dec:expectExitCode  0 ;
    dec:captureOutput   true .

<step:2> a dec:VerificationStep ;
    dec:stepType        "sparql-assertion" ;
    dec:target          ".dec/store" ;
    dec:query           "SELECT ?s WHERE { ?s a dec:ValueStream }" ;
    dec:expectRows      1 .
```

`dec:verifies` is polymorphic: its range is `dec:Feature ∪ dec:TC`. The same TC may be verified by multiple graphs in different environments; the slice-3 aggregate verdict composes their outcomes.

### Typed step vocabulary

Slice-2.5 seed types, each with its own SHACL shape and declared `dec:requiredOps`:

| Step type | `dec:requiredOps` | Purpose |
|---|---|---|
| `shell-command` | `shell`, `filesystem` | Run a command; assert exit code and/or stdout pattern. |
| `sparql-assertion` | `sparql-local` (file target) or `sparql-http` (endpoint target) | Run a SPARQL query; assert row count or specific values. |
| `file-assertion` | `filesystem` | Assert file existence, content equality, or hash. |
| `http-request` | `http` (safe verbs) or `http-mutating` (POST/PUT/DELETE) | Make an HTTP call; assert status and response shape. |
| `wait-for` | union of the wrapped condition's ops | Poll a sub-condition with timeout. |
| `capture` | — | Bind a prior step's stdout/result to a name. |

Step bodies may contain `${name}` references to prior `capture` bindings. **The `${name}` syntax is reserved in slice 2.5; resolution lands in slice 3.** Authoring tools accept the literal string but do not interpret it; the slice-3 executor performs substitution.

Later slices may extend the registry (`dagger-pipeline`, `git-state`, `metric-window`, `llm-judgment`). Extensions land as separate features that add a SHACL shape and an executor entry — the ADR does not need amendment unless a new step type is judgment-laden enough to alter the verifier role's contract.

### Coverage predicate: `dec:providesEvidenceFor`

Each `dec:VerificationStep` carries an **optional** `dec:providesEvidenceFor` predicate whose object is a TC IRI. Multiple values are allowed: a single step (e.g. a `shell-command` that compiles + tests + asserts) may provide evidence for several TCs.

```turtle
<step:2> a dec:VerificationStep ;
    dec:stepType        "sparql-assertion" ;
    dec:target          ".dec/store" ;
    dec:query           "SELECT ?s WHERE { ?s a dec:ValueStream }" ;
    dec:expectRows      1 ;
    dec:providesEvidenceFor <tc:TC-007>, <tc:TC-008> .
```

This predicate is what makes **coverage structural and queryable**. A feature `F` is covered by a graph `G` iff for every TC `T` in `F.tests`, some step in `G` declares `dec:providesEvidenceFor T`. Coverage becomes a SPARQL query, not a free-text match against the TC body.

The predicate is OPTIONAL on the SHACL shape — graphs authored before this predicate existed remain SHACL-valid; their measured coverage is simply zero until annotated. This preserves slice-2.5 forward-compatibility.

The predicate is consumed by:

- [ADR-030](ADR-030)'s verify-graph-author role (the coverage report it returns is computed over this predicate).
- [ADR-031](ADR-031)'s chain-integrity dispatch gate (it refuses dispatch when uncovered TCs exist).
- Slice 3's graph executor (it can map per-step trace back to TC verdicts using the same predicate).

[FT-036](FT-036) carries the SHACL change; FT-036 is still `planned`, so the shape update lives in its body directly and no separate amendment feature is needed.

### Safety gating

The authoring CLI and the dispatch handler refuse to persist or dispatch a graph whose `step.requiredOps ⊄ env.allowedOps`. The check is structural and runs twice:

- **At authoring time.** `dec verify step add` and its MCP twin (per [ADR-CLI-MCP](ADR-029)) refuse to save the artifact. The error names the specific op and the offending step.
- **At pre-dispatch time.** Defensive replay; the orchestrator re-validates in case the env or graph mutated since authoring. A mid-flight violation produces a `VerificationVerdict` of `rejected` with `dec:violates` naming the safety constraint.

Safety class composes with autonomy levels ([docs/ddd/DDD_and_the_Five_Levels_of_AI_Autonomy.md](docs/ddd/DDD_and_the_Five_Levels_of_AI_Autonomy.md)):

- `isolated` graphs may execute autonomously up through Level 4.
- `shared-non-destructive` graphs require explicit dispatch approval at Level 3+.
- `production-readonly` graphs always require a human checkpoint regardless of role autonomy.

A verifier role authored at Level 4 against an isolated environment runs unattended; the same role asked to verify in production drops back to a checkpointed gate. The autonomy level is a property of the role; the safety class is a property of the environment; the orchestrator takes the stricter of the two.

### Multi-graph aggregation

A single TC may be referenced by multiple `VerificationGraph`s — one ephemeral, one dev, one prod. The slice-3 aggregate `VerificationVerdict`:

- `approved` iff every graph's verdict is `approved`,
- `amendment-required` iff any graph is `amendment-required` and none are `rejected`,
- `rejected` iff any graph is `rejected`.

Partial-pass cases (e.g. "local approved, production pending") are first-class in `dec verify check FT-XXX` output. The aggregation rule lives in the slice-3 executor; the per-graph verdict shape is unchanged from [ADR-018](ADR-018).

### Storage format

Raw Turtle, not markdown-with-frontmatter. Environments and graphs are structured data with little prose; markdown frontmatter would be a near-empty body wrapping the real artifact. Turtle is honest about that.

Visual rendering of the graph (ASCII DAG, web view) is a slice-3+ concern. Authoring is text-editor-friendly: each artifact is a small Turtle file, browsable with `dec verify graph show VG-NNN` (which renders the Turtle as a step table).

### Relationship to FT-023's worker

Slice-3 graph executor with per-step trace makes [FT-023](FT-023)'s single-shot LLM verifier worker redundant: verification becomes "execute the graph, capture per-step trace, aggregate per-step outcomes, emit verdict" — no separate LLM verifier process. **FT-023's `workers/verifier/` package is superseded by this ADR when slice 3 closes.** Any judgment-laden steps later expressible as an `llm-judgment` step type re-enter through the graph, not as a parallel worker. [ADR-020](ADR-020)'s "single-shot LLM for slice 2" remains the correct decision for the slice it governs; this ADR records the next-slice direction.

## Rejected alternatives

- **TCs carry the procedure inline.** Rejected — TCs are declarative ("the property must hold"); procedures are operational ("here is how to demonstrate the property"). Conflating them prevents one TC from having multiple verification graphs across environments, which is the central composition this ADR enables.
- **Untyped step bodies (free-form text the LLM interprets at runtime).** Rejected — defeats safety gating (no `requiredOps` to check), defeats replay (each LLM run reinterprets), defeats the manual-to-automatic transition (still a rewrite when execution lands). Typed steps with explicit `requiredOps` is what makes the orchestrator able to refuse unsafe dispatch.
- **Single global environment per repo.** Rejected — production verification needs different `allowedOps` than ephemeral verification. Multi-environment is the whole point.
- **Markdown-with-frontmatter storage (like product features).** Rejected — bodies would be essentially empty; the structure *is* the artifact. Turtle is closer to the truth.
- **Extend `product` to own verification graphs.** Rejected — product-cli describes *what to build*; verification procedure is *how the engineering process operates*, which is dec's concern (`CLAUDE.md §The principle that governs everything`).

## Consequences

**Positive:**
- Verification becomes composable: same TC, multiple graphs across environments, aggregate verdict.
- Safety gating composes with autonomy levels at a single chokepoint (the dispatch handler).
- Manual-to-automatic transition is mechanical: a slice-2.5 authored graph runs unchanged once the slice-3 executor lands.
- The verifier role becomes environment-aware, closing one of the open questions implicit in [ADR-019](ADR-019)'s independence framing.
- Every typed step has explicit ops, which makes the dispatch-time safety check a simple subset test rather than an inferred analysis of an opaque worker.

**Negative / accepted costs:**
- Two new artifact types to learn and maintain.
- The single-shot LLM verifier worker ([FT-023](FT-023)) is on a deprecation path; the migration to graph-executor verification is a multi-slice transition.
- Authoring overhead per feature: in addition to writing a TC, contributors author a `VerificationGraph`. Slice 2.5 makes this optional; a later slice may make it required via a gap-check fitness function.

**Enforcement:**
- SHACL shapes for `VerificationEnvironment`, `VerificationGraph`, and each step type — embedded in the ontology bundle, validated at `StreamWriter` commit time.
- The authoring CLI (and its MCP twin per [ADR-CLI-MCP](ADR-029)) refuses violating saves before persistence.
- A later TC (slice 3) reports features with no `VerificationGraph` as a structural gap.

## Status

Proposed. Supersedes [ADR-020](ADR-020)'s slice-2 single-shot worker direction once slice 3 lands; ADR-020 remains the correct decision for the slice it governs. Bound to slice 2.5 (FT-A through FT-D — artifact types, authoring surface, safety enforcement) and slice 3 (executor, dispatch integration, aggregate verdict).
