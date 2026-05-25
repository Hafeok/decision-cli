---
id: ADR-052
title: Structured LLM output via instructor + Pydantic
status: accepted
features:
- FT-081
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:e1c14fb5952b043130b44b34a5523b7977ac06ea7c8114d3abcb02a8db6f17aa
---

## Context

LLM responses need to map reliably to artifact builder calls — a worker that
calls `provider.complete(output_schema=ADRSchema, …)` must get back something
that constructs an `ADRSchema` without ad-hoc JSON parsing. Three families
of solutions:

- **Provider-native structured output:** Anthropic tool use, OpenAI
  `response_format: json_schema`, Scaleway's variant. Each provider has its
  own surface; works well but differs.
- **instructor + Pydantic:** library that wraps provider clients, normalizes
  the schema-coercion behavior across providers, retries on parse failure,
  works on top of OpenAI-compatible APIs (which LiteLLM provides).
- **BAML:** stronger guarantees via a separate schema language and runtime
  enforcer, but introduces a build-time DSL.

## Decision

instructor + Pydantic, layered on top of LiteLLM's OpenAI-compatible
endpoint. Provider-native structured output is used under the hood where
LiteLLM supports it (Anthropic tool use, OpenAI `response_format`); the SDK
exposes one uniform Pydantic-based surface.

## Consequences

- **Positive:** One schema-coercion behavior across all providers. The
  worker's `output_schema=` parameter behaves the same regardless of which
  concrete model LiteLLM routes to.
- **Positive:** Pydantic models are also what the artifact builders (FT-080)
  consume — direct hand-off from LLM response to builder.commit() with no
  intermediate translation.
- **Positive:** Retry-on-validation-failure is built-in (instructor's
  retry loop with validation errors fed back to the model).
- **Negative:** instructor adds a dependency. Mitigated by it being a thin
  library with a stable API surface.
- **Negative:** Where provider-native structured output is meaningfully
  better than what instructor exposes, we lose that. Mitigated by
  `extra_body` passthrough — workers can drop down to provider-specific
  features when warranted.

## Alternatives considered

- **Provider-native structured output, exposed directly.** Rejected: each
  provider's surface is different, workers would need provider-aware code,
  defeats the capability-tag abstraction.
- **BAML.** Stronger guarantees but introduces a DSL and a build step
  outside the Python ecosystem the worker SDK lives in. Revisit if
  instructor proves structurally insufficient.

## References

- `feature:provider-abstraction` (FT-081) uses instructor for structured
  output.
- ADR-054 (LiteLLM as substrate) provides the OpenAI-compatible endpoint
  instructor wraps.