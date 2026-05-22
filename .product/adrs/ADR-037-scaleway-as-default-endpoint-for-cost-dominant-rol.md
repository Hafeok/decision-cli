---
id: ADR-037
title: Scaleway as default endpoint for cost-dominant roles, Anthropic reserved for deep-reasoning escalation
status: accepted
features:
- FT-058
- FT-059
- FT-065
supersedes: []
superseded-by: []
domains:
- api
- networking
- security
scope: cross-cutting
content-hash: sha256:274614c9b4bcf56fe05af9b346978feeb698c9786072adc3b33d3bde73a3366e
amendments:
- date: 2026-05-22T18:48:18Z
  reason: Updated PRD §5.2 corrects Opus 4.7 pricing to $5/$25 per M tokens (escalation cost ratio ~6-8× vs tier-2 Scaleway, not 25×). Adds candidate-status Anthropic capabilities (`mid-reasoning` Sonnet 4.6, `fast-reasoning` Haiku 4.5) as A/B targets the meta-loop can promote without an ADR amendment. Adds the cache-hit-rate fitness function (target >70% on escalated Anthropic sessions, via [FT-065](FT-065)) which materially changes the deep-reasoning cost calculus.
  previous-hash: sha256:481d30a0890298a19c27a0c0e426e4ee8d52a548129eb127b335315831d0146e
---

## Context

[ADR-033](ADR-033) introduces capability-based model routing as a mechanism; this ADR records the policy choice that mechanism enables: **Scaleway's OpenAI-compatible serverless inference endpoint as the default for cost-dominant roles, with Anthropic reserved for deep-reasoning escalation.**

The numbers force the question. At sustained workload (~8h/day on the implementer role) running everything on Claude Opus burns budget on cases a smaller model would handle equally well. The price asymmetry from the PRD's catalog (§5.2), with prompt-caching for Anthropic:

| Capability tier | Endpoint | Model | Currency | Input | Output | Cache-hit input |
|---|---|---|---|---|---|---|
| 1 (code-writer default) | Scaleway | qwen3-coder-30b-a3b-instruct | EUR | 0.20 | 0.80 | — |
| 2 (heavy code, frontier reasoning) | Scaleway | devstral-2-123b / qwen3.5-397b | EUR | 0.40–0.60 | 2.00–3.60 | — |
| 3 (deep-reasoning escalation) | Anthropic | claude-opus-4-7 | USD | 5.00 | 25.00 | 0.50 |

(Candidate Anthropic capabilities `mid-reasoning` Sonnet 4.6 at $3/$15/$0.30, `fast-reasoning` Haiku 4.5 at $1/$5/$0.10 sit in the catalog as A/B targets — see "Candidate capabilities" below.)

Tier 1 input is ~25× cheaper than tier 3 base input (or ~2.5× cheaper than tier 3 cache-hit input). Output is ~30× cheaper. For the dominant case (incremental implementation passes that hit the bundle's exit criteria first try), tier 1 is the right call; for the genuinely hard case (foundational schema design, deep architectural rationale), tier 3 is the right call. The cost-dominant claim is empirical at this point — current Phase 2 work runs almost entirely on Claude — but the framework's substrate is what makes it *possible* to route differently.

The risk: Scaleway models may be lower quality than Claude on tasks the framework cares about. The framework's structural answer is escalation ([ADR-034](ADR-034)) — if the verifier's confidence drops below 0.7 or an audit fails, the dispatcher promotes the work to a stronger tier. The Anthropic path remains intact; what changes is that it is reached via escalation, not as the default.

A second risk: Scaleway's API is a third-party dependency new to this codebase. Rate limits, availability, regional issues, OpenAI-compatibility quirks (PRD §14 calls out the resolved questions on `reasoning_effort` and reasoning-trace exposure). The mitigation is that Scaleway is *one* endpoint among several — the catalog is endpoint-extensible, and the Anthropic path is not removed.

See the parent PRD: §1 (Scaleway integration), §5.2 (catalog with endpoint column and Anthropic cache pricing), §10 (Scaleway client integration), §13 (risks).

## Decision

The seed catalog ([FT-058](FT-058), [ADR-036](ADR-036)) binds cost-dominant roles to Scaleway by default and reserves Anthropic for tier-3 deep-reasoning escalation:

| Role | Default capability | Endpoint at default | First escalation | Deep escalation |
|---|---|---|---|---|
| implementer | `code-writer` (qwen3-coder-30b) | Scaleway | `code-writer-heavy` (devstral-2-123b, Scaleway) | `deep-reasoning` (opus-4-7, Anthropic) |
| verifier | `code-writer` (qwen3-coder-30b) | Scaleway | `standard-reasoning-frontier` (qwen3.5-397b, Scaleway) | `deep-reasoning` (opus-4-7, Anthropic) |
| architect | `standard-reasoning` (gpt-oss-120b) | Scaleway | `standard-reasoning-frontier` (qwen3.5-397b, Scaleway) | `deep-reasoning` (opus-4-7, Anthropic) |
| test_interpreter | `classifier` (mistral-small-3.2-24b) | Scaleway | — | — |
| feedback_class_triager | `classifier` (mistral-small-3.2-24b) | Scaleway | — | — |

Bounded-classification roles (`test_interpreter`, `feedback_class_triager`) bind to the `classifier` capability with empty escalation chains — these are mechanical classification tasks where escalation is structurally inappropriate.

### Candidate capabilities

The seed catalog also includes two `candidate`-status Anthropic capabilities not currently bound to any role:

- `mid-reasoning` — Sonnet 4.6 ($3/$15 per M; $0.30/M cache hits).
- `fast-reasoning` — Haiku 4.5 ($1/$5 per M; $0.10/M cache hits).

These are available for the meta-loop to promote as A/B test targets without authoring new `Capability` artifacts — the catalog is one-line policy edits all the way down. Plausible promotion paths if Scaleway-only proves insufficient on quality:

- Verifier's first escalation step rebinds from `standard-reasoning-frontier` (Scaleway) to `mid-reasoning` (Anthropic Sonnet) for a measurement window. The cost premium is ~5× vs the Scaleway frontier, but Sonnet caching at $0.30/M softens that on escalation chains (see [FT-065](FT-065)).
- A new bounded classification role binds to `fast-reasoning` instead of `classifier` if Mistral's classifier quality lags.

Neither needs an ADR amendment; the catalog already declares both capabilities, and a role binding revision is a one-line policy change.

### Configuration

- `SCW_SECRET_KEY` environment variable holds the Scaleway secret key, stored in the same secrets mechanism as `ANTHROPIC_API_KEY` (whatever is currently in use; the framework does not prescribe a secret manager).
- Base URL is fixed at `https://api.scaleway.ai/v1` ([FT-059](FT-059)).
- `dec doctor` reports presence of both `SCW_SECRET_KEY` and `ANTHROPIC_API_KEY` so operators can diagnose missing-key dispatch failures up front.

### Endpoint policy invariants

- **No silent fallback between endpoints.** A Scaleway dispatch that fails (network error, rate limit) does not transparently retry on Anthropic. Escalation is a *capability change* driven by signals, not by endpoint availability.
- **Catalog can reference any endpoint.** This ADR fixes only the *default* bindings. The catalog itself is extensible per [ADR-036](ADR-036) — adding a new endpoint or rebinding roles is a graph mutation, not an ADR amendment.
- **Cost-dominant claim is testable.** A Phase 3 measurement TC compares aggregate cost per dispatch under default bindings vs. all-Anthropic bindings on a fixed cohort. If the gap is < 5×, the policy is reconsidered.

### Cache-hit rate as fitness function

Anthropic dispatches in escalation chains can take advantage of prompt caching (see [FT-065](FT-065) and PRD §9.4). The dispatcher places a cache breakpoint between the bundle's stable prefix (focal artifact + ADRs + tools) and the per-attempt suffix (prior-attempt enrichment block). The first dispatch in a chain pays the cache-write rate; subsequent dispatches within 5 minutes pay the cache-hit rate (10× cheaper on Opus, 10× cheaper on Sonnet and Haiku as well).

**Target: > 70% cache-hit rate on escalated Anthropic sessions** where the bundle prefix is stable. Track as a fitness function on the dispatcher. A persistent rate below 70% indicates the cache breakpoint is misplaced (per-attempt content leaking into the prefix); the breakpoint placement is then revisited as a follow-up tuning.

The cache pricing matters for the cost calculus: with cache hits, escalating to Opus on the third tier of a chain costs roughly the same per-token as the second-tier Scaleway dispatch did per-token. Without caching, the gap is ~6-8× per token (still cheaper than the older Opus 4.1 pricing this PRD initially cited — see "Pricing correction" below).

### What this ADR does not decide

- **Cross-region failover.** Out of scope (PRD §3); handled at the worker layer if needed, same as Anthropic rate-limit handling today.
- **Per-feature endpoint overrides.** Not supported — bindings are role-stable. A feature_spec that demands a specific endpoint must override at the bundle-composition level by setting an unusual capability binding for the dispatch, which is a structural smell.
- **Cache breakpoint placement strategy beyond a single breakpoint.** Currently one breakpoint between stable prefix and per-attempt suffix. If escalation chains grow longer or bundles grow more complex, additional breakpoints (Anthropic supports up to 4) become a follow-up optimization.

## Consequences

**Positive.**

- Cost drops substantially under normal workload. The default path is tier-1 Scaleway; tier-3 Anthropic is reserved for cases where the framework's own signals say "this is hard".
- Anthropic remains in the catalog as the deep-reasoning floor; the framework does not lose its strongest reasoning capability — it just stops paying for it on every dispatch.
- Prompt caching on Anthropic chains drops the marginal cost of deep escalation by 10× per cache-hit token. Bundle prefixes are mostly static across escalation tiers, so this is a real cost mover.
- Endpoint extensibility is preserved by construction. A future provider (e.g. self-hosted vLLM on the operator's hardware) is a Capability entry + client wrapper, not an architectural change.
- Bounded-classification roles get appropriately small models (mistral-small-3.2-24b) instead of paying Sonnet/Opus rates for what is effectively a labeling task.
- Candidate capabilities sit in the catalog as A/B targets; the meta-loop can promote them via a single role-binding revision without any ADR amendment or new artifact authoring.

**Negative / accepted costs.**

- New external dependency: Scaleway's API, with its own SLA, rate limits, and OpenAI-compatibility quirks. The framework now depends on two providers being healthy for default dispatch.
- Calibration risk on the confidence threshold. `confidence_below_0.7` is the PRD's initial value; if Scaleway verifier confidence is systematically lower than Claude verifier confidence (different model calibration), the threshold may need to drop. This is the measurement work [ADR-034](ADR-034)'s escalation policy is designed to enable, but it does require running the system long enough to gather evidence.
- Scaleway model quality may be lower on edge cases. The escalation path catches *known* failure modes (audit_fail, low confidence, repeated attempts); silent-quality-drop failure modes (a verifier that confidently approves a bad implementation) are caught only at the next stage of verification (CI, human review).
- Cache-hit rate below target indicates a tuning problem (breakpoint placement). The dispatcher must track this and surface it; without measurement, the cache strategy degenerates into noise.
- Operator setup gains a step: obtain a Scaleway API key, add to env. The cost is one-time per operator; `dec doctor` surfaces missing keys.

### Pricing correction

The PRD's initial cost table cited Opus 4.1 pricing (~$13.50/$67.50 per M tokens). The updated PRD §5.2 corrects this to the Opus 4.7 pricing of $5/$25 per M, with cache-hit input at $0.50/M. The escalation cost ratio of tier-3 vs tier-2 Scaleway is therefore ~6-8× per token (without caching) or roughly equivalent per token (with caching on a stable prefix), not the ~25× this ADR originally implied. The structural conclusion — escalation is meaningful cost but not catastrophic — is unchanged.

**Boundary enforcement.**

- The default bindings live in the seed YAML ([ADR-036](ADR-036)); reviewing the YAML is reviewing the policy.
- A `Capability` artifact's `endpoint` field is constrained by SHACL to a known vocabulary (`scaleway`, `anthropic` initially); adding an endpoint requires an ontology extension.
- The Anthropic path is never removed from the catalog — `deep-reasoning` (opus-4-7) is always reachable via escalation, not by config flag.
- Cache-hit rate is a measured fitness function, not an unverified claim.

## Relationship to existing ADRs

- **[ADR-008](ADR-008) (worker contract).** Preserved — workers receive the resolved `(endpoint, model, params)` triple and call accordingly; they do not encode endpoint policy.
- **[ADR-020](ADR-020) (verifier single-shot).** Compatible — the verifier remains single-shot. What changes is the model identifier arrives via dispatch payload from the resolved capability.

## Status

Proposed. Governs the *content* of [FT-058](FT-058) (catalog bootstrap seed values), the Scaleway client integration in [FT-059](FT-059), and the Anthropic prompt caching feature [FT-065](FT-065). The default bindings themselves are catalog entries; this ADR is the rationale for why they are what they are. Pricing correction and cache-hit fitness function recorded by amendment.
