---
id: ADR-080
title: Apply DDD task/cell decomposition to decision-cli's self-implementation pipeline; broad code-writer becomes the unknown-task path
status: accepted
features:
- FT-139
- FT-142
- FT-140
- FT-141
- FT-143
- FT-144
- FT-150
- FT-154
- FT-156
- FT-163
- FT-164
- FT-165
- FT-166
supersedes: []
superseded-by: []
domains:
- api
scope: platform
content-hash: sha256:2430e55f5b441d3ed2ea145fcc435f14d2a295f1d6ff6d02031000906b932b4d
---

## Context

The standard shape of LLM-driven code generation is a single broad-authority worker handed a feature and a tool belt — `claude -p` historically, the FT-123 in-process LiteLLM agentic loop today. One worker holds the whole problem, makes every kind of decision (infrastructure, API shape, test strategy, error handling, naming) in one session, drifts when steered wrong, and produces a black-box decision graph that cannot be audited.

The *applications/sdlc.md* document in the DDD framework derives this as the textbook case of the "fusion the one-prompt-one-artifact-type rule forbids": one role making many kinds of decisions in one session. DDD's prescription is that **code is an artifact like any other** — a unit of implementation is a cluster of typed sub-artifacts, each its own `(role, artifact type)` cell with its own prompt, generation decisions, and model binding. The single steered agent decomposes into a small graph of single-purpose generators.

The witnessed roadmap proves the pattern fits decision-cli's own self-implementation pipeline almost embarrassingly well. Looking at planned features only:

| Witnessed task type | Examples (shipped + planned) |
|---|---|
| Add a Python judge/author worker | FT-126 tc-author (shipped), FT-127 tc-quality (shipped), FT-128 vg-quality, FT-129 spec-author, FT-130 adr-author, FT-132 spec-quality, FT-133 adr-quality |
| Add a new artifact type | FT-026 Feedback, FT-035 VerificationBench, FT-054 Capability, FT-071 BoundaryArtifact, FT-086 WorkerImage |
| Add a CLI subcommand | FT-038, FT-049, FT-099, FT-109, FT-110 |
| Extend a planner classifier | FT-131, FT-138 (just shipped), FT-119 |
| Extend the role catalog seed | FT-068, FT-121 (just shipped), FT-066-deferred-migration |

For two of these (FT-126, FT-127) the diffs of consecutive worker-package implementations are 80% identical boilerplate — `pyproject.toml`, agent loop calling LiteLLM with the same shape, Pydantic input/output models, system prompt under `src/foo/prompts/`, capability binding, role catalog seed. Each ride through the broad code-writer re-derives this from scratch and consumes the same cost. The third (FT-128) would do it again. So would the fourth, fifth, sixth.

The current dispatcher routes every feature through the broad code-writer regardless of whether the task is recognized. That's correct when the task is genuinely unknown — the SDLC doc's analysis is that the broad worker's principled standing is exactly as the "explorer-and-typifier" that owns the unknown path. It is wasteful, unaudited, and slow when the task type is one we have shipped twice already.

## Decision

**Adopt the task/cell decomposition from `applications/sdlc.md` for decision-cli's self-implementation pipeline. The broad code-writer becomes the unknown-task fallback; recognized task types dispatch their declared cell clusters with a per-task coherence audit.**

Concretely:

### 1. Introduce TaskType + Cell to the orchestration vocabulary

A **TaskType** declares: a name, a recognition signature (so the classifier can match a feature to it), an ordered cluster of Cells (with `derived_from` ordering), and a **coherence audit** (named, executable, owned by the TaskType).

A **Cell** declares: a name, an artifact type it produces, a prompt template, a model binding, and the upstream cells it derives from.

Initial substrate: TaskType + Cell live as feature_spec bodies in the proposed catalog under the convention `FT-TT-<name>` (task type) and `FT-CELL-<name>` (cell), with `phase: 5` and `domains: [api, observability]`. Promotion to first-class product-cli artifact types is itself an "add-an-artifact-type" task and is deferred — the bootstrap problem resolves naturally once the artifact-type task type is itself dispatched as a cluster.

### 2. Classifier with explicit broad-worker escape hatch

`dec drive ship` gains a classification step before dispatch:

- **Known TaskType match (high confidence)** → dispatch the cell cluster.
- **Known TaskType match (low confidence)** OR **unknown** → dispatch the broad code-writer (existing path).

Confidence is a per-classifier concern; the v1 classifier uses explicit `task_type: <name>` declaration in the feature_spec front-matter as the high-confidence signal, with the broad worker covering everything else. Future versions may add schema-shape signatures, embedding similarity, or LLM-based classification — out of scope here.

The escape hatch is non-negotiable per the SDLC doc: *"Misclassification dispatches a confidently-wrong cluster, so the escape hatch matters."* It must not be an error path; it must be a first-class branch.

### 3. First TaskType: `add-judge-worker` — load-bearing prototype

The SDLC doc is explicit: *"The coherence audit is the load-bearing audit of the whole pattern — worth prototyping first. If it is weaker than what a single context gave for free, the decomposition is worse than the monolith."*

We prototype the audit on **`add-judge-worker`** because:
- The cluster has the most witnesses (2 shipped + 3 planned).
- The audit is concrete and testable: the agent_loop's LiteLLM call shape must agree with the capability_binding's `endpoint`/`model_id` must agree with the pydantic_io_models' input schema must agree with the unit_tests' fixture payload. Four artifacts, one shared contract, one mechanical SHACL/SPARQL check.
- The funnel inside the cluster is small (prompt cell may want reasoning; agent loop wants code-specialist; capability binding is mechanical) — easy to validate model-binding-by-cell.

If the audit catches a divergence the broad worker would have caught for free in a shared context, the cluster proves it has teeth. If it cannot, the decomposition is worse than the broad worker and we revisit.

### 4. Mixed-feature composition is the normal case

Per the SDLC doc, no feature is purely one task type. A "add a judge worker" feature ships its worker cluster *plus* a role-catalog-seed-extension cluster *plus* maybe an MCP-tool-registration cell. The dispatcher composes clusters within a feature; the coherence audit runs across the union.

### 5. Sub-task type expansion follows the prototype

Once `add-judge-worker`'s audit shows teeth, expand to the other witnessed types (each its own feature_spec, drafted in parallel): `add-author-worker`, `add-artifact-type`, `add-cli-subcommand`, `extend-planner-classifier`, `extend-role-catalog-seed`.

## Rejected alternatives

### Keep the monolith — every feature through the broad worker

Status quo. Rejected — wastes compute on routine patterns, produces an unauditable decision graph per the SDLC doc analysis, and the witnessed roadmap proves the pattern fixpoint already exists. Continuing to absorb the cost is engineering on inertia.

### Specialize prompts per task type but keep one worker

A simpler intervention: detect the task type and swap in a specialized system prompt for the broad worker, without decomposing into cells. Rejected — preserves the fused decision graph (one session, many decisions); does not unlock model binding per cell; cannot run a structural coherence audit because the contract surface across decisions is still implicit. This is the "make the existing worker smarter" path that the SDLC doc explicitly forecloses: *"a muddied decision graph hiding inside a single worker."*

### Add TaskType + Cell as first-class product-cli artifact types now

Cleanest from a graph-discipline perspective but bootstraps badly: adding an artifact type *is* one of the witnessed task types, so we'd be using the cell-cluster pattern on its own foundations. Rejected for v1 — TaskType + Cell live as feature_spec bodies under a naming convention initially; promotion to artifact types happens once the artifact-type task type is itself dispatched as a cluster, naturally closing the bootstrap loop.

### Skip the broad worker entirely

Cell clusters for everything; if no cluster matches, error out. Rejected — eliminates the explicit unknown-task path, which the SDLC doc identifies as the principled standing of the broad-authority worker. Misclassification has no escape; truly novel work has nowhere to land. The escape hatch is the pattern, not a workaround.

### Cell-cluster without coherence audit

Decompose into cells, dispatch in `derived_from` order, but rely on the typed artifacts agreeing implicitly. Rejected by the SDLC doc directly: *"The fix is the shared upstream artifact (the contract) plus a named coherence audit owned by the task type. This is the load-bearing audit of the whole pattern — worth prototyping first."* Skipping the audit is shipping the pattern without its safety property.

### Defer until product-cli grows TaskType + Cell artifact types

Wait for the cleaner foundation. Rejected — the 5 planned worker features (FT-128..FT-133) are queued *now*; each one shipped through the broad worker is a load-bearing prototype opportunity wasted. The feature_spec body convention is sufficient for the v1 cluster; the proper artifact-type addition lands as one of the cell-cluster task types in due course.

## Consequences

### Positive

- **The broad worker retires from common patterns.** FT-128..FT-133 dispatch cheaply through the `add-judge-worker` and `add-author-worker` clusters, with model binding per cell following the funnel inside the feature.
- **The coherence audit replaces the broad worker's "shared context"** — explicit, structural, testable. The unaudited monolith's main strength (everything agrees because everything sees everything) becomes an explicit check that fails loudly when it doesn't hold.
- **The decision graph becomes walkable.** Each cell's prompt + bundle + emitted artifact + model binding is recorded as its own session; the cluster is a graph of sessions, not a black-box blob.
- **Maturation curve becomes visible.** As more witnessed types fold into the TaskType catalog, the broad worker's share of dispatches drops measurably. Per the SDLC doc, "80% of features decompose into known task types" is the type-decomposability fitness function — and we can now measure it.
- **Misclassification has an explicit escape.** The broad worker is the documented `not_confident → broad worker` branch, not an error path.

### Negative / accepted trade-offs

- **`derived_from` ordering becomes explicit.** Each TaskType declares its cell order; the dispatcher honours it. No more "the agent will figure it out" — the order is data, not emergent. Cost: every TaskType author must articulate ordering; benefit: ordering is reviewable and reproducible.
- **The coherence audit is a new failure surface.** It can be wrong (false positive blocks a legitimate cluster) or absent (false negative permits drift). The prototype on `add-judge-worker` is precisely the test of whether it has the right teeth.
- **The cell-cluster dispatcher is new code on the `dec drive ship` path.** New module, new state machine for cluster execution, new failure modes (partial-cluster-shipped, audit-failed-after-cells-landed, etc.). Scope kept small in the first slice by limiting to one TaskType.
- **Classifier confidence is a soft signal.** v1 relies on operator-declared `task_type:` in the feature_spec. The escape hatch absorbs the misclassification cost; better classifiers are follow-on slices.
- **Bootstrap cost.** TaskType + Cell don't exist yet; the first slice ships the substrate before the first cluster runs. Mitigated by scoping the substrate minimally and prototyping on a single TaskType.

### Relationship to prior decisions

- **[FT-123](FT-123)** retired the `claude -p` subprocess in favour of in-process LiteLLM in the broad code-writer. That's the broad worker this ADR keeps. FT-123 made the broad worker first-class; this ADR makes the typed alternative first-class alongside it.
- **[FT-066](FT-066) / [FT-067](FT-067) / [FT-068](FT-068)** migrated verify-graph-author through the capability resolver. The cell-cluster pattern absorbs this — each cell's model binding comes from a capability binding the same way. The deferred code-writer migration becomes a cell in the `extend-role-catalog-seed` task type.
- **[FT-110](FT-110) / [FT-119](FT-119)** give us the `dec drive` planner shape. The classifier + cluster dispatcher land as an extension to the existing planner registry, not a parallel system.
- **[ADR-070](ADR-070) / [ADR-071](ADR-071) / [FT-121](FT-121)** established `dec:roleTool` as the per-role tool surface. Each cell's prompt declares its tool needs; the cluster's role binding ANDs them.

## Status

Proposed. Promotes to accepted once FT-139 ships the substrate + first TaskType + coherence audit prototype, and the audit demonstrably catches at least one cluster divergence the broad worker would have caught implicitly.
