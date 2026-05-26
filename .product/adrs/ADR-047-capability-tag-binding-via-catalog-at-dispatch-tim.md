---
id: ADR-047
title: Capability-tag binding via catalog at dispatch time (workers never see model names)
status: accepted
features:
- FT-081
- FT-061
- FT-080
supersedes: []
superseded-by: []
domains:
- api
scope: domain
content-hash: sha256:8441d0248ada479fa450ff849c8e66b75ea332b9966a8a3f26a1ec91d5cc4b7b
---

## Context

Role-to-model bindings can reference either:

- **Model names directly** (`"anthropic/claude-sonnet-4-5"`,
  `"scaleway/qwen-3-32b-instruct"`): simple, but every model change requires
  edits to every place that names it.
- **Capability tags** (`"frontier-reasoning"`, `"code-specialized"`,
  `"fast-cheap"`): abstract, requires a catalog to resolve at dispatch time,
  but model swaps become catalog edits.

The decision-cli capability layer (FT-054…FT-058, ADR-033) already commits to
capability tags as the binding currency for orchestrator-side routing. The
worker SDK must align.

## Decision

The SDK's `Provider` layer consumes capability tags from dispatch events and
resolves them via the catalog — concretely, capability tags map to LiteLLM
model groups (per ADR-054), and LiteLLM does the model-name lookup. Workers
never see model names.

Per-call shape:

```python
response = await provider.complete(
    capability_tag="frontier-reasoning",   # resolved by LiteLLM
    messages=[...],
    output_schema=ADRSchema,
    metadata={"ddd_session_id": session.id},
)
```

## Consequences

- **Positive:** New model qualifies → catalog update (LiteLLM proxy config) →
  no SDK or worker change required. The "models change weekly, roles change
  rarely" assumption from ADR-033 holds end-to-end.
- **Positive:** The same dispatch can route through different concrete models
  in different deployments (dev, staging, prod) by varying the LiteLLM
  config, not the code.
- **Negative:** Workers cannot make model-specific assumptions (max context
  window, supported tool-use shape, etc.) in code. This is correct — model-
  specific assumptions belong in the capability tag's contract, which is in
  the catalog. But it does demand discipline.

## Alternatives considered

- **Hardcoded model names in workers.** Rejected: makes the model catalog
  decorative; rebinding requires code changes; defeats the purpose of
  capability tags.
- **Capability tags resolved client-side in the SDK** (the SDK reads the
  catalog directly and picks a model). Rejected: duplicates LiteLLM's
  routing logic, fragments routing across SDK instances. Better to let the
  proxy own one routing source of truth (see ADR-054).

## References

- `feature:provider-abstraction` (FT-081) implements capability-tag dispatch.
- ADR-033 (capability-based model routing as graph-resident layer).
- ADR-054 (LiteLLM as substrate).