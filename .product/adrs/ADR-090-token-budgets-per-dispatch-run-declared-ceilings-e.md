---
id: ADR-090
title: 'Token budgets per dispatch run: declared ceilings enforced by the harness between dispatches, from existing session accounting'
status: accepted
features: []
supersedes: []
superseded-by: []
domains:
- error-handling
- observability
scope: domain
content-hash: sha256:16d0f4f444c9f8b09eae692e6acf126fcaad290c238c9efee50dccc42b3321c3
---

**Status:** Proposed

## Context

Every cost control in the dispatch machinery today bounds something other than tokens:

- **Turn caps** (`max_turns`, 40 for cluster cells per [FT-164](FT-164)) bound the *number* of LLM calls, not their size. Forty turns over a growing agentic context is unbounded token spend by design — the late turns are the expensive ones.
- **Wall-clock timeouts** (1800s per cell, [FT-170](FT-170); per-call 240s worker-side) bound latency, which correlates with tokens only loosely.
- **Repair-round caps** ([FT-171](FT-171): 2 audit-repair rounds, 2 per-cell retries) bound *attempts*, each of which re-spends the full bundle.
- **Escalation tiers** ([FT-062](FT-062)) actively *increase* spend on failure — the chain's response to a failed attempt is a more expensive model over an enriched (larger) bundle.

Meanwhile the accounting already exists and is precise: [FT-146](FT-146) records a four-way token breakdown per cell (`input_tokens_base`, `input_tokens_cache_write`, `input_tokens_cache_hit`, `output_tokens`) on the cluster SessionRecord, and the worker reports the same usage structure on every dispatch. We measure spend exactly and bound it nowhere. The exposure is concrete: a single pathological cluster run (wide TaskType × repair rounds × 40-turn cells) can consume tens of millions of tokens while every existing cap reports healthy, and provider rate limits (`docs/scaleway-rate-limits.md`: 400k TPM on the current code-writer tier) turn that spend into throughput starvation for every concurrent run sharing the key.

The pipeline factory tracks context budget cumulatively across retries per step and acts on thresholds — warn at 200% of budget, hard-fail at 500%. The principle to borrow: **a declared cumulative ceiling, evaluated where the money is actually spent, with a soft warning band before the hard stop**. The factory's anchor (percent-of-context-window) fits its single-conversation steps; our runs span many sessions over many models, so the natural denomination is absolute tokens per run.

## Decision

**Dispatch runs carry declared token budgets. The harness evaluates cumulative spend from the existing usage accounting at every between-dispatch boundary; crossing the soft threshold records a warning, crossing the hard ceiling aborts the run cleanly with a structured, graph-resident failure.**

1. **Two budgeted run scopes.** A *cluster run* (all cells, all retries, all repair rounds of one `cluster_dispatch` invocation) and an *escalation chain* (all tier attempts of one `dispatch_role` invocation). These are the two places the harness loops over LLM dispatches.
2. **Budgets are declared in the graph, with policy defaults.** A TaskType may declare its cluster-run budget; a RoleBinding may declare its chain budget; an orchestration-store policy artifact supplies defaults ([ADR-036](ADR-036) catalogs-in-graph). Compiled fallback defaults apply on legacy stores. A budget of `unlimited` must be declared explicitly to opt out — absence means the default, not infinity.
3. **Spend is the paid-token sum.** Budget consumption is `input_tokens_base + input_tokens_cache_write + output_tokens`, summed across every dispatch in the run scope. Cache hits are excluded from consumption (an order of magnitude cheaper; excluding them keeps the cache breakpoints of [FT-065](FT-065) cost-aligned rather than budget-punished) but remain recorded and reported as today.
4. **Enforcement at between-dispatch boundaries.** The harness checks the budget before dispatching the next cell, the next retry, the next repair round, or the next escalation tier. In-flight calls are never interrupted — `max_turns` and per-call limits keep a single dispatch bounded; the budget bounds the loop.
5. **Soft threshold warns, hard ceiling aborts.** Crossing the soft threshold (default 50% of the ceiling) records a budget-warning on the session record and a tracing warning; the run continues. Reaching the ceiling aborts the run as a structured failure — `budget-exceeded`, carrying spent/declared per class — persisted on the SessionRecord like any other terminal failure. A cluster aborts with its sandbox preserved (same operator surface as audit-cap exhaustion); a chain returns its failure to the caller.
6. **Spend and verdict are queryable.** `dec session show` renders budget state (declared, spent, outcome) for cluster and chain sessions from the graph alone.

## Rationale

- **It closes the only unbounded axis.** Turns, wall-clock, rounds, and tiers are each capped; tokens — the thing actually billed and rate-limited — were not. The budget converts "healthy caps, runaway bill" into a clean, attributable failure.
- **Zero new accounting.** FT-146 already counts every class per dispatch; the decision is purely to *act* on numbers we already persist. Enforcement is a comparison in the two existing loops.
- **Between-dispatch enforcement matches authority boundaries.** The harness owns the loop; the worker owns one dispatch. Aborting between dispatches needs no new worker contract and never produces a half-written cell — the same clean-boundary reasoning as [FT-170](FT-170)'s placement snapshots.
- **Soft-then-hard mirrors proven practice.** The factory's two-band scheme gives the operator a recorded early signal before work is lost; a single hard cliff would either be set too generously to warn or too tightly to finish legitimate runs.

## Rejected alternatives

- **Keep indirect caps only (status quo).** Turn and time caps demonstrably do not bound spend; the gap grows with TaskType width and escalation depth.
- **Provider rate limits as the control.** TPM limits are shared across all concurrent runs on a key, unattributable to a run, and their failure mode is throttling/429 churn, not a recorded decision; they protect the provider, not the operator's budget.
- **Mid-call interruption (streaming cutoff).** Kills a dispatch after its input tokens are already paid, yields unusable partial output, and needs new worker-contract machinery. Per-call bounds already exist; the loop is the unbounded part.
- **Monetary budgets (currency, not tokens).** Strictly more meaningful but requires complete, current price data for every capability; the capability catalog's cost fields are optional today. Token ceilings are provider-neutral and computable now; a monetary layer can be derived later from the same recorded spend.
- **One global budget across all runs (daemon-level).** There is no resident daemon in slice 1 to own a global ledger, and a global pot lets one runaway run starve every other; per-run scopes match the dispatch atoms and keep failures attributable.

## Test coverage

- A cluster run whose next cell would follow ceiling-exceeding spend aborts with a structured `budget-exceeded` SessionRecord; the sandbox is preserved.
- Crossing the soft threshold records a warning and the run continues to completion.
- An escalation chain stops before the next tier once the chain budget is exhausted, returning the structured failure.
- Budgets resolve from TaskType/RoleBinding declarations, then the policy artifact, then compiled defaults on a legacy store; explicit `unlimited` opts out.
