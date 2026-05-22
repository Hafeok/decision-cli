---
id: ADR-033
title: Capability-based model routing as a graph-resident layer
status: accepted
features:
- FT-054
- FT-055
- FT-057
- FT-060
- FT-061
- FT-064
- FT-065
supersedes: []
superseded-by: []
domains:
- api
- data-model
- observability
scope: cross-cutting
content-hash: sha256:b4117bf950723c269f15e7eecf8ef51dd0518ddc4af4add4a6fb78ab88aceaab
amendments:
- date: 2026-05-22T18:46:11Z
  reason: Updated PRD adds candidate-status Anthropic capabilities (`mid-reasoning` Sonnet 4.6, `fast-reasoning` Haiku 4.5) to the seed catalog as A/B targets for the meta-loop; Opus 4.7 pricing corrected to $5/$25 per M tokens (not $13.50/$67.50 — the older Opus 4.1 numbers); record the `candidate` status semantics so role bindings can promote them later via a one-line policy change, and add fields cost_currency / cost_cache_hit_per_m / cost_cache_write_5m / exposes_reasoning_trace to the Capability schema this ADR governs.
  previous-hash: sha256:480b8cb252ede99d98253e1482813e124b45cc586b39ef11dfabaa7bc9676d36
---

## Context

Slice-2 workers carry their model bindings inline. The verifier worker (`workers/verifier/src/verifier/worker.py:17`) declares `DEFAULT_MODEL_ID = "claude-sonnet-4-5"` and routes through the `anthropic` SDK directly. The code-writer worker ([FT-013](FT-013)) delegates to `claude -p` headless subprocesses — the model identity is whatever the operator's Claude Code installation resolves at runtime. [FT-030](FT-030) records role *authorities* (what roles may decide) but not what model *executes* each role.

This is three problems trying to be one:

1. **Workers know about models.** The worker contract from [ADR-008](ADR-008) says workers are stateless `bundle → artifact` functions; they should not encode operator-side policy about which model is appropriate for which work. Today they do, because the model id is a string the worker reads from its own constants/env-var fallbacks.
2. **Model selection is a code change.** Swapping the verifier from Sonnet to Opus requires editing `worker.py`. The DDD thesis — different model behind each role, evolved through a policy artifact — is currently a slogan; the substrate to act on it does not exist.
3. **No measurement substrate.** The meta-loop's normal pattern (measure role-model fit → propose binding change → validate → apply) requires bindings to be graph data. Today they are Python constants.

[ADR-015](ADR-015) (graph-native worker bindings) addressed *which executable* runs a role. This ADR addresses the orthogonal axis: *which model* runs through that executable. The two concerns must not be conflated — a single `code-writer` binary in slice 3 will route to multiple models depending on the bundle.

See the parent PRD: §1 (capability layer thesis), §2 (motivation), §3 (scope), §5 (catalog schema).

## Decision

Introduce two new artifact types and a resolution boundary:

1. **`dec:Capability`** — a versioned binding from a stable tag (e.g. `code-writer`, `standard-reasoning`, `deep-reasoning`) to an `(endpoint, model_identifier, parameters)` triple plus declared model properties (context window, tool-calling support, cost, tier, status). The catalog *is* a set of `dec:Capability` artifacts in the graph. See [FT-054](FT-054).
2. **`dec:RoleBinding`** — declares the *default capability* for a role plus an ordered list of escalation steps. See [FT-055](FT-055) and [ADR-034](ADR-034) for the escalation half.

**The dispatcher resolves role → capability → (endpoint, model, params) at dispatch time.** Worker dispatch payloads carry the resolved triple as concrete strings/numbers; workers consume what the dispatcher provides and remain ignorant of capabilities, role bindings, escalation, and cost.

This explicitly cleaves three concerns that were tangled:

| Concern | Lives in | Examples |
|---|---|---|
| Which executable runs role R? | `dec:WorkerBinding` (ADR-015) | the code-writer uv-tool at version 0.4.2 |
| Which model is appropriate for role R given this bundle? | `dec:RoleBinding` + `dec:Capability` (this ADR) | implementer defaults to `code-writer` capability (qwen3-coder-30b on Scaleway) |
| What does the worker *do* once it has a model? | Worker code | bundle parse, prompt build, model call, output validation |

The capability boundary is enforced by the dispatch payload contract: workers see `endpoint: "scaleway" | "anthropic"`, `model_identifier: "<exact API string>"`, `parameters: {…}`. They do *not* see a `capability_id`, a binding, or escalation context. That information stays in the dispatcher and the graph.

## Consequences

**Positive.**

- Per-role model choice becomes a one-line policy revision (rewrite the `dec:RoleBinding`'s `default_capability` or escalation order), not a worker code edit.
- The catalog is graph-native; the meta-loop can read it, propose changes, and validate them against the same authoring path as any other artifact.
- Session records can cite the `Capability` (id + version) that ran them, joining model identity to the PROV-O chain — closing a gap that was implicit in [ADR-004](ADR-004) but never written.
- Adding endpoints (Scaleway, future providers) becomes a Capability catalog change plus a client wrapper; it does not require touching the role catalog, the dispatcher core, or worker contracts.
- The worker contract from [ADR-008](ADR-008) is preserved — strengthened, actually, because the model identity is now *injected* into the dispatch payload by the dispatcher instead of being a worker constant.

**Negative / accepted costs.**

- A new resolution step in the dispatcher's hot path. Pre-resolution adds a SPARQL lookup per dispatch (acceptable: bindings change orders of magnitude less often than dispatches; memoising the active binding per role is straightforward).
- The verifier's `ModelCaller` indirection (already present at `worker.py:52`) must accept an endpoint discriminant, not just a model id. That seam is small but real.
- Operators who today understand "Sonnet is the verifier model" must learn one more level of indirection ("the verifier *role* binds to the `code-writer` *capability* which resolves to qwen3-coder-30b on Scaleway"). The catalog is the cure for the gap, but it is also one more thing to read before understanding why a session used a particular model.
- Misconfigured bindings can produce confident-looking but wrong dispatches (e.g. binding implementer to a `vision-general` capability with no tool support). Mitigation: SHACL on `dec:RoleBinding` enforces capability compatibility (tool-calling required for implementer/verifier), and the dispatcher refuses to dispatch when the resolved capability fails capability-shape validation.

**Boundary enforcement.**

- `dec:WorkerBinding` (ADR-015) stays scoped to executable identity. It does *not* gain model fields.
- The dispatcher is the only graph reader of `RoleBinding`/`Capability`. Workers never query the graph (ADR-008 stands).
- The dispatch payload schema is the contract; growing it requires a feature_spec and an ADR amendment, not a worker-side workaround.

## Relationship to existing ADRs

- **[ADR-008](ADR-008) (worker contract).** Unchanged in substance; the contract is *bundle → artifact*. This ADR adds that the dispatch payload now also carries the resolved `(endpoint, model, params)` triple alongside the bundle. Worker statelessness and bundle-completeness are preserved.
- **[ADR-015](ADR-015) (graph-native worker bindings).** Orthogonal. Worker bindings answer *which binary runs*; capability bindings answer *which model the binary calls*. The two artifacts coexist on a session: `session.workerBinding` + `session.capability` (with version pins on both) form the full reproducibility tuple.
- **[ADR-020](ADR-020) (verifier single-shot).** Unchanged in substance. The verifier remains a single-shot call; what changes is that the model id arrives via dispatch payload rather than being a module constant.
- **[ADR-027](ADR-027) (authority declarations).** Orthogonal. Authority describes what a role *may decide*; capability describes *which model executes that decision*. A role with `mayDecide: ["verdict-classification"]` may still bind to multiple capabilities across escalation steps.

## Amendment — capability schema additions and candidate-status capabilities

The updated PRD expands the `dec:Capability` schema and the seed catalog. Both changes preserve the orthogonality this ADR establishes; they refine the model identity record without altering how capability binding works.

### Schema additions

The `dec:Capability` class gains four properties, documented in detail in [FT-054](FT-054):

- `dec:cost_currency` (required, enum `EUR` | `USD`). Scaleway invoices in EUR, Anthropic in USD; recording the unit per-capability avoids stale conversion values baked into session records. Cross-endpoint cost rollups convert at query time using a recorded exchange rate, not at dispatch time.
- `dec:cost_cache_hit_per_m` (optional, decimal). For endpoints with prompt caching, the input rate for cached prefix tokens. Currently set only on Anthropic capabilities — the cache-hit rate is 10× cheaper than base input on Opus 4.7 ($0.50/M vs $5.00/M), which makes cache breakpoints worthwhile on escalation chains (see [FT-065](FT-065)).
- `dec:cost_cache_write_5m` (optional, decimal). The 5-minute cache TTL write rate, slightly above base input on Anthropic.
- `dec:exposes_reasoning_trace` (optional, default `false`). True for capabilities that emit a separate reasoning chain alongside content — currently `qwen3.5-397b-a17b` (`standard-reasoning-frontier`) is the only such capability in the seed catalog. The worker parses `response.choices[0].message.reasoning` for those capabilities and attaches the chain as `rationale_trace` evidence to the produced artifact (see [FT-060](FT-060) §10.6).

These extensions stay inside the worker contract from [ADR-008](ADR-008): workers receive the resolved capability triple plus parameters, and respond to the `exposes_reasoning_trace` field by parsing one additional API response field. No graph access, no policy decisions.

### Candidate-status capabilities

The seed catalog adds two `candidate`-status Anthropic capabilities not bound to any current role:

- `mid-reasoning` — Sonnet 4.6 ($3/$15 per M, with $0.30/M cache hits).
- `fast-reasoning` — Haiku 4.5 ($1/$5 per M, with $0.10/M cache hits).

The `candidate` status (defined in [FT-054](FT-054)'s SHACL enum) means *ready for promotion if measurement evidence supports it*. Sitting in the catalog as `candidate` means a meta-loop session can flip them to `active` and update a role binding without authoring a new `Capability` artifact. This makes the existing meta-loop substrate (rebind a role to a different capability) the only mechanism for promotion — there is no "new capability arrives" pipeline distinct from the meta-loop's normal pattern.

This formalises one of the central claims of this ADR: model selection is policy, not code. Adding a model to the candidate slate is a YAML/catalog edit (one feature_spec for the catalog update); promoting a candidate to a default capability is a `dec:RoleBinding` revision (one feature_spec for the binding change). Neither requires a worker code change or an ADR amendment — provided the new candidate fits within the existing capability schema, which is why the schema additions above are deliberately narrow.

### Pricing correction

The PRD's initial Anthropic pricing reference cited Opus 4.1 numbers ($13.50/$67.50). The updated PRD §5.2 corrects this to the Opus 4.7 pricing of $5/$25 per M input/output tokens, with $0.50/M cache-hit input. The cost ratio of escalating to deep-reasoning vs staying on tier-2 Scaleway is therefore ~6-8× (not 25×). This is still meaningful cost — Opus stays the escalation floor, not the default — but it narrows the dynamic range and informs the calibration of `confidence_below_*` thresholds on the verifier binding.

## Status

Proposed. Governs [FT-054](FT-054) (Capability), [FT-055](FT-055) (RoleBinding), [FT-061](FT-061) (dispatcher capability resolution), [FT-064](FT-064) (migration cleanup), [FT-065](FT-065) (Anthropic prompt caching). Companion to [ADR-034](ADR-034) (escalation policy), [ADR-035](ADR-035) (bundle stakes), [ADR-036](ADR-036) (graph-native catalog), [ADR-037](ADR-037) (endpoint policy). Schema additions and candidate-status capabilities recorded by amendment.
