---
id: ADR-030
title: Verify-graph-author role and graph-proposal output
status: proposed
features: []
supersedes: []
superseded-by: []
domains: []
scope: domain
---

## Context

[ADR-028](ADR-028) established verification as a typed graph × typed environment. Slice 2.5 ([FT-035](FT-035)–[FT-044](FT-044)) gives humans (and LLMs via MCP) the verbs to author those artifacts by hand: `dec verify env new`, `dec verify graph new`, `dec verify step add`. That works, but it puts authoring effort on the human for every feature, and it admits the failure mode where a feature is authored, TCs are written, and the implementer is dispatched — but **nobody ever wrote a graph to verify the work**.

The natural mechanism to close that loop is symmetric to the implementer ([ADR-008](ADR-008), [FT-013](FT-013)): an LLM-backed role that reads a feature_spec + its TCs and produces a `dec:VerificationGraph` artifact. The implementer produces code; the verify-graph-author produces the verification artifact. Both feed downstream interpretation steps ([ADR-017](ADR-017) action-interpretation pairing): the implementer's `CodeChange` is interpreted by a verifier; the author's `VerificationGraph` is interpreted by the slice-3 graph executor.

This ADR fixes the role's *shape* (bundle, output, persistence path, autonomy level). It does **not** fix the dispatch trigger; that's [ADR-031](ADR-031) (chain-integrity gate).

## Decision

A new role, `verify-graph-author`, is introduced. It is a stateless LLM-backed worker that takes a feature and proposes one `dec:VerificationGraph` per call. Concretely:

1. **Bundle contract.** The harness assembles a `VerifyGraphAuthorInput` containing:
   - `feature_id` and the feature_spec body.
   - The full set of TCs the feature references (each with id, body, and any structured acceptance fields).
   - The catalog of available `dec:VerificationEnvironment` artifacts the worker may target (with `env_type`, `safety_class`, `allowed_ops`, and optional `endpoint`).
   - The catalog of **existing** `dec:VerificationGraph` artifacts that already reference any of the feature's TCs (for match-or-generate).
   - The step vocabulary the worker is allowed to use: the six seed kinds from [FT-036](FT-036) and their `requiredOps`.
   - A `bundle_hash` over the canonical serialisation of the above.

2. **One graph per environment per call.** A single invocation produces at most one graph. The caller specifies a target environment (`--environment ENV-NNN`); the worker's job is "design a graph in *this* env that covers as many of the feature's TCs as possible". Multi-environment coverage is achieved by invoking the worker once per env. This keeps the worker's decision logic linear and avoids the worker having to schedule across environments.

3. **`GraphProposal` output.** The worker returns a Pydantic structure that is exactly one of:
   - `Match { graph_id: VG-NNN, coverage_report }` — an existing graph already covers the feature's TCs in this env; do not create a new one.
   - `New { environment: ENV-NNN, steps: [TypedStep, ...], coverage_report }` — a new graph spec ready to be written.
   - `Gap { uncovered_tcs: [TC-NNN], reason: "step vocabulary insufficient for X" }` — the worker cannot, in good faith, produce a graph that covers the feature's TCs with the available step vocabulary. This is feedback ([ADR-022](ADR-022)/[ADR-023](ADR-023) `class: gap`), not a graph.

   The harness inspects the proposal and either writes the new graph through the [FT-036](FT-036) writer, records the match, or routes the gap upstream.

4. **Coverage is structural.** A graph covers a feature iff every TC in `feature.tests` is referenced by some `dec:VerificationStep` via `dec:providesEvidenceFor TC-NNN`. The amendment to [ADR-028](ADR-028) (see "Consequences" below) adds this predicate to the step shapes; coverage is then a SPARQL query, not a free-text match against the TC body. The primitive lives in `core::verify::coverage::*` ([FT-045](FT-045)).

5. **Match-or-generate is a deterministic precondition, not a worker decision.** The harness computes `best_matching_graph` ([FT-046](FT-046)) *before* invoking the worker. If a complete match exists for this env, the worker is not invoked and the caller receives a `Match`. The worker only sees the candidate set so it can extend the rationale of its `New` proposal (e.g. "I propose extending the pattern from VG-007 because it covers TC-051 the same way"). The decision logic is in Rust, not in the LLM prompt.

6. **Persistence path.** When the harness accepts a `New` proposal, it writes the graph via [FT-041](FT-041)'s `dec verify graph new` handler followed by [FT-044](FT-044)'s `dec verify step add` handler per step. **No new write path** is introduced; the author worker reuses the slice-2.5 writers. This preserves the [ADR-029](ADR-029) single-handler discipline and ensures CLI, MCP, and worker-driven persistence all go through one chokepoint.

7. **Level-3 autonomy.** Per [docs/ddd/DDD_and_the_Five_Levels_of_AI_Autonomy.md](docs/ddd/DDD_and_the_Five_Levels_of_AI_Autonomy.md), the role operates at Level 3: the worker proposes; a human accepts or rejects the proposal before persistence. The CLI surface ([FT-049](FT-049)) prints the proposal and prompts for confirmation; the MCP twin returns the proposal as a structured value and writes only on a separate `--accept` (or `accept=true`) follow-up call. Subscription-triggered dispatch ([FT-050](FT-050)) raises a session in `pending_review` state rather than persisting; reviewers act on it via the existing session inspection commands. Graduation to Level 4 (auto-persist when confidence is high) is out of scope here and will be a separate ADR once we have empirical agreement-rate data ([ADR-021](ADR-021) fitness function).

8. **Read-only access to the graph.** Like all workers, verify-graph-author has **no direct graph access** ([CLAUDE.md](CLAUDE.md) workers section). Everything it needs is in the bundle. The catalogs (envs, existing graphs, step vocab) are inlined; the worker does not call SPARQL.

## Consequences

**Positive:**

- Coverage becomes a query, not a vibe. `feature_covered_by(FT, VG)` is a SPARQL CONSTRUCT over `dec:providesEvidenceFor`. The slice-3 graph executor and the slice-2.6 chain gate ([ADR-031](ADR-031)) both consume the same primitive.
- One bundle in, one structured output out, no graph access — the same SDP shape that has held for the implementer and the verifier ([ADR-008](ADR-008), [ADR-020](ADR-020)). The author worker is the third instance of this pattern, which reinforces it.
- The harness, not the LLM, decides match-vs-generate. Determinism where it belongs.
- Level 3 is honest about what we can verify today (agreement metric is not yet calibrated for this role). The persistence-after-acceptance pattern means we can't accidentally pollute the graph store with low-quality proposals.

**Negative / accepted trade-offs:**

- One-graph-per-call means features whose TCs span environments require multiple invocations. The orchestrator (or the human at the CLI) is responsible for the env loop. The alternative (worker plans across envs) was rejected as a complexity multiplier with no clear win.
- Persistence reuses the slice-2.5 writers, so the worker cannot author a graph + add a hundred steps atomically. Each step-add round-trips through SHACL and the StreamWriter. This is correct (the chokepoint is real) but slow for very large graphs. If profiling shows this matters, a future ADR can introduce a "batch step add" handler — but the single-handler discipline must still hold (one CLI verb, one MCP tool, one Rust function).
- Level 3 means a human is in the loop for every persistence. That's the right default; until we have a baseline agreement rate we should not graduate. The cost is reviewer time; the benefit is no auto-corruption of the verification graph store.

**Amendment to [ADR-028](ADR-028):** Step shapes carry an optional `dec:providesEvidenceFor` predicate whose object is a TC IRI. Multiple values allowed (one step may provide evidence for several TCs). This predicate is what makes coverage structural and queryable. The amendment is recorded via `product adr amend ADR-028`; the SHACL shape change lives in [FT-036](FT-036), which is still `planned` and editable.

**Forward references:**

- [ADR-031](ADR-031) — chain-integrity invariant; gates dispatch on coverage path existence.
- [FT-045](FT-045)–[FT-050](FT-050) — implementation features of slice 2.6.
- A future ADR (post slice 3) — autonomy graduation criteria for verify-graph-author once `dec:Agreement` metric data is available.
