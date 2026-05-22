---
id: ADR-037
title: Scaleway as default endpoint for cost-dominant roles, Anthropic reserved for deep-reasoning escalation
status: accepted
features:
- FT-058
- FT-059
supersedes: []
superseded-by: []
domains:
- api
- networking
- security
scope: cross-cutting
content-hash: sha256:481d30a0890298a19c27a0c0e426e4ee8d52a548129eb127b335315831d0146e
---

## Context

[ADR-033](ADR-033) introduces capability-based model routing as a mechanism; this ADR records the policy choice that mechanism enables: **Scaleway's OpenAI-compatible serverless inference endpoint as the default for cost-dominant roles, with Anthropic reserved for deep-reasoning escalation.**

The numbers force the question. At sustained workload (~8h/day on the implementer role) running everything on Claude Opus burns budget on cases a smaller model would handle equally well. The price asymmetry from the PRD's catalog (§5.2):

| Capability tier | Endpoint | Model | €/M in | €/M out |
|---|---|---|---|---|
| 1 (code-writer default) | Scaleway | qwen3-coder-30b-a3b-instruct | 0.20 | 0.80 |
| 2 (heavy code, frontier reasoning) | Scaleway | devstral-2-123b / qwen3.5-397b | 0.40–0.60 | 2.00–3.60 |
| 3 (deep-reasoning escalation) | Anthropic | claude-opus-4-7 | ~13.50 | ~67.50 |

Tier 1 is ~67× cheaper input and ~84× cheaper output than tier 3. For the dominant case (incremental implementation passes that hit the bundle's exit criteria first try), tier 1 is the right call; for the genuinely hard case (foundational schema design, deep architectural rationale), tier 3 is the right call. The cost-dominant claim is empirical at this point — current Phase 2 work runs almost entirely on Claude — but the framework's substrate is what makes it *possible* to route differently.

The risk: Scaleway models may be lower quality than Claude on tasks the framework cares about. The framework's structural answer is escalation ([ADR-034](ADR-034)) — if the verifier's confidence drops below 0.7 or an audit fails, the dispatcher promotes the work to a stronger tier. The Anthropic path remains intact; what changes is that it is reached via escalation, not as the default.

A second risk: Scaleway's API is a third-party dependency new to this codebase. Rate limits, availability, regional issues, OpenAI-compatibility quirks (the PRD's open questions in §14 specifically call out `reasoning_effort` and reasoning-trace exposure). The mitigation is that Scaleway is *one* endpoint among several — the catalog is endpoint-extensible, and the Anthropic path is not removed.

See the parent PRD: §1 (Scaleway integration), §5.2 (catalog with endpoint column), §10 (Scaleway client integration), §13 (risks).

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

### Configuration

- `SCW_SECRET_KEY` environment variable holds the Scaleway secret key, stored in the same secrets mechanism as `ANTHROPIC_API_KEY` (whatever is currently in use; the framework does not prescribe a secret manager).
- Base URL is fixed at `https://api.scaleway.ai/v1` ([FT-059](FT-059)).
- `dec doctor` reports presence of both `SCW_SECRET_KEY` and `ANTHROPIC_API_KEY` so operators can diagnose missing-key dispatch failures up front.

### Endpoint policy invariants

- **No silent fallback between endpoints.** A Scaleway dispatch that fails (network error, rate limit) does not transparently retry on Anthropic. Escalation is a *capability change* driven by signals, not by endpoint availability.
- **Catalog can reference any endpoint.** This ADR fixes only the *default* bindings. The catalog itself is extensible per [ADR-036](ADR-036) — adding a new endpoint or rebinding roles is a graph mutation, not an ADR amendment.
- **Cost-dominant claim is testable.** A Phase 3 measurement TC compares aggregate cost per dispatch under default bindings vs. all-Anthropic bindings on a fixed cohort. If the gap is < 5×, the policy is reconsidered.

### What this ADR does not decide

- **Cross-region failover.** Out of scope (PRD §3); handled at the worker layer if needed, same as Anthropic rate-limit handling today.
- **Caching.** No caching layer for repeated bundles (PRD §3).
- **Per-feature endpoint overrides.** Not supported — bindings are role-stable. A feature_spec that demands a specific endpoint must override at the bundle-composition level by setting an unusual capability binding for the dispatch, which is a structural smell.

## Consequences

**Positive.**

- Cost drops substantially under normal workload. The default path is tier-1 Scaleway; tier-3 Anthropic is reserved for cases where the framework's own signals say "this is hard".
- Anthropic remains in the catalog as the deep-reasoning floor; the framework does not lose its strongest reasoning capability — it just stops paying for it on every dispatch.
- Endpoint extensibility is preserved by construction. A future provider (e.g. self-hosted vLLM on the operator's hardware) is a Capability entry + client wrapper, not an architectural change.
- Bounded-classification roles get appropriately small models (mistral-small-3.2-24b) instead of paying Sonnet/Opus rates for what is effectively a labeling task.

**Negative / accepted costs.**

- New external dependency: Scaleway's API, with its own SLA, rate limits, and OpenAI-compatibility quirks. The framework now depends on two providers being healthy for default dispatch.
- Calibration risk on the confidence threshold. `confidence_below_0.7` is the PRD's initial value; if Scaleway verifier confidence is systematically lower than Claude verifier confidence (different model calibration), the threshold may need to drop. This is the measurement work [ADR-034](ADR-034)'s escalation policy is designed to enable, but it does require running the system long enough to gather evidence.
- Scaleway model quality may be lower on edge cases. The escalation path catches *known* failure modes (audit_fail, low confidence, repeated attempts); silent-quality-drop failure modes (a verifier that confidently approves a bad implementation) are caught only at the next stage of verification (CI, human review).
- Operator setup gains a step: obtain a Scaleway API key, add to env. The cost is one-time per operator; `dec doctor` surfaces missing keys.

**Boundary enforcement.**

- The default bindings live in the seed YAML ([ADR-036](ADR-036)); reviewing the YAML is reviewing the policy.
- A `Capability` artifact's `endpoint` field is constrained by SHACL to a known vocabulary (`scaleway`, `anthropic` initially); adding an endpoint requires an ontology extension.
- The Anthropic path is never removed from the catalog — `deep-reasoning` (opus-4-7) is always reachable via escalation, not by config flag.

## Relationship to existing ADRs

- **[ADR-008](ADR-008) (worker contract).** Preserved — workers receive the resolved `(endpoint, model, params)` triple and call accordingly; they do not encode endpoint policy.
- **[ADR-020](ADR-020) (verifier single-shot).** Compatible — the verifier remains single-shot. What changes is the model identifier arrives via dispatch payload from the resolved capability.

## Status

Proposed. Governs the *content* of [FT-058](FT-058) (catalog bootstrap seed values) and the Scaleway client integration in [FT-059](FT-059). The default bindings themselves are catalog entries; this ADR is the rationale for why they are what they are.
