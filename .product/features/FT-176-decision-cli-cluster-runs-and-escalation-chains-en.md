---
id: FT-176
title: 'decision-cli: cluster runs and escalation chains enforce declared token budgets from session accounting'
phase: 5
status: planned
depends-on:
- FT-171
adrs:
- ADR-090
- ADR-036
- ADR-008
tests:
- TC-449
- TC-450
- TC-451
- TC-452
domains:
- error-handling
- observability
domains-acknowledged:
  ADR-084: No archetype ships or changes status; seam audits are unaffected by spend enforcement.
  ADR-083: A budget ceiling is run policy resolved at dispatch, not a tech detail binding at archetype/instance/feature level.
  ADR-081: Budget state renders inside existing session show output; no new enumerate/lookup verb pair.
  ADR-082: Budgets attach to TaskType/RoleBinding as run policy; archetype contracts and audit scopes are unaffected.
  ADR-087: Budget enforcement sits between dispatches; audit emission and repair targeting are untouched (FT-173's slice).
---

## Description

Implements [ADR-090](ADR-090): the two harness dispatch loops — cluster runs (`cluster_dispatch`) and escalation chains (`dispatch_role`) — gain declared cumulative token budgets, evaluated between dispatches from the usage accounting that already flows through every worker response ([FT-146](FT-146)). Crossing the soft threshold records a warning; reaching the hard ceiling aborts the run with a structured `budget-exceeded` failure persisted on the SessionRecord. The factory analogue is per-step cumulative context budgets with warn/fail bands; ours is denominated in absolute paid tokens because a run spans many sessions over many models.

## Functional Specification

### Inputs

- Per-dispatch usage (`input_tokens_base`, `input_tokens_cache_write`, `input_tokens_cache_hit`, `output_tokens`) from worker responses — already recorded per [FT-146](FT-146).
- Budget declarations: TaskType (cluster-run ceiling), RoleBinding (chain ceiling), orchestration-store policy artifact (defaults), compiled defaults (legacy stores). Explicit `unlimited` opts out.
- Soft-threshold fraction (default 50% of ceiling) from the same policy resolution.

### Outputs

- A budget evaluator in the harness: spend = Σ(`input_tokens_base` + `input_tokens_cache_write` + `output_tokens`) over the run scope; cache hits excluded from consumption, still recorded.
- Budget checks before each cell dispatch, cell retry, repair round ([FT-171](FT-171)), and escalation tier ([FT-062](FT-062)).
- SessionRecord extensions: resolved budget + source, soft-threshold warning record, structured `budget-exceeded` terminal failure with spent-vs-declared per class.
- `dec session show` renders budget state (declared, spent, outcome) for cluster and chain sessions.

### State

- New quads on existing cluster/chain session records; one new policy artifact shape in the orchestration store ([ADR-036](ADR-036)). No new artifact types beyond the policy shape.

### Behaviour

1. The budget is resolved once at run start (declaration → policy → compiled default) and recorded with its source.
2. Spend accumulates as each worker response lands; the check runs at every between-dispatch boundary — in-flight calls are never interrupted.
3. Soft crossing: warning quad + tracing warn; run continues. Ceiling reached: cluster aborts preserving the sandbox (operator surface identical to audit-cap exhaustion); chain returns the structured failure without dispatching the next tier.
4. `unlimited` disables the check for that run and is recorded as the resolved budget.

### Invariants

- No run scope dispatches with an unresolved budget — absence of declarations means the default, never infinity.
- Enforcement never truncates an in-flight dispatch; a single dispatch stays bounded by `max_turns` and per-call limits.
- The spend recorded on the session record equals the sum of the per-dispatch usage records in the same scope (no second ledger).

### Error handling

- A malformed policy artifact degrades to compiled defaults with a warning quad, never to an open gate.
- Workers reporting no usage (legacy/stub) count zero toward spend but the absence is recorded — silent zero would mask under-reporting.

### Boundaries

- The worker contract is untouched: usage reporting stays exactly the FT-146 shape; budgets are harness-side only.
- Drive-sweep (`--all`) level budgets and monetary budgets are explicitly deferred (ADR-090 rejected alternatives).

## Out of scope

- Cross-run or daemon-global budget ledgers.
- Monetary/currency budgets derived from capability cost fields.
- Provider rate-limit coordination (TPM smoothing, backoff tuning).
- Budget-aware planning (e.g. choosing cheaper tiers under pressure) — the budget is a stop, not a scheduler.
