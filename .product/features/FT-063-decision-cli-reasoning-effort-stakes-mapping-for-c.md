---
id: FT-063
title: 'decision-cli: reasoning_effort stakes mapping for configurable_effort capabilities'
phase: 2
status: planned
depends-on:
- FT-056
- FT-061
adrs:
- ADR-035
tests:
- TC-112
domains:
- api
domains-acknowledged: {}
---

## Description

When the dispatcher resolves a role to a capability with `configurable_effort = true` (e.g. `standard-reasoning` / gpt-oss-120b per [FT-054](FT-054)), compute the `reasoning_effort` model parameter from the bundle's `stakes` field per [ADR-035](ADR-035) and inject it into the dispatch payload's `parameters.reasoning_effort`. The mapping is fixed: `routine → low`, `elevated → medium`, `foundational → high`. For capabilities without `configurable_effort`, the dispatcher leaves `reasoning_effort` null and the worker ignores it.

This is the smallest of the new features but the most concrete demonstration of why the per-bundle stakes judgment from [FT-056](FT-056) and the per-capability properties from [FT-054](FT-054) work together: a single field on the bundle drives a model parameter on a specific capability without code branches in the worker.

## Functional Specification

### Inputs

- `ResolvedCapability` from [FT-061](FT-061) with `configurable_effort: bool`.
- `Bundle.stakes` from [FT-056](FT-056).
- The dispatch payload assembly point in `core::dispatcher::compute_params` ([FT-061](FT-061)).

### Outputs

- New function `core::dispatcher::params::compute_reasoning_effort(stakes: Stakes, configurable_effort: bool) -> Option<&'static str>`:
  ```rust
  pub fn compute_reasoning_effort(stakes: Stakes, configurable_effort: bool) -> Option<&'static str> {
      if !configurable_effort { return None; }
      Some(match stakes {
          Stakes::Routine => "low",
          Stakes::Elevated => "medium",
          Stakes::Foundational => "high",
      })
  }
  ```
- Dispatch payload's `parameters.reasoning_effort` populated from this function. Where the capability is not `configurable_effort`, the field is absent / `null`.
- The Scaleway client wrapper from [FT-059](FT-059) consumes `reasoning_effort` from `params` and includes it in the API request. The PRD's open question on whether `reasoning_effort` is a top-level field or an `extra_body` key on the OpenAI-compat shape is resolved empirically at [FT-059](FT-059) integration time; this feature relies on whichever path [FT-059](FT-059) lands.

### State

- No state. Pure function plus a payload-assembly call.

### Behaviour

1. After [FT-061](FT-061) resolves the capability, `compute_params` is called.
2. `compute_params` invokes `compute_reasoning_effort(bundle.stakes, resolved.configurable_effort)`.
3. The result is placed at `params.reasoning_effort` in the dispatch payload.
4. The worker's `ModelRouter` (from [FT-060](FT-060)) forwards the value to the API call.
5. For Anthropic capabilities (which do not have `reasoning_effort`), `configurable_effort` is false and the field is `null` — the Anthropic router ignores it.

### Mapping table

The mapping is a fixed property of the framework, not per-binding policy:

| `bundle.stakes` | `reasoning_effort` |
|---|---|
| `routine` | `low` |
| `elevated` | `medium` |
| `foundational` | `high` |

Note: For the architect role binding, `stakes = foundational` *also* triggers escalation to `standard-reasoning-frontier` per [FT-058](FT-058)'s seed bindings. In practice the default capability (`standard-reasoning`) sees `routine` and `elevated`; `foundational` reaches it only if a later operator removes the escalation step. The `foundational → high` mapping is preserved for completeness.

### Invariants

- `compute_reasoning_effort(_, false) == None` for every input. The function does not produce a value when the capability does not accept it.
- The function is total: every `Stakes` variant maps to a defined value.
- The function is referentially transparent — same inputs always yield same output, no graph reads.

### Error handling

- None — the function cannot fail. An unknown stakes value is impossible (the enum is closed via [FT-056](FT-056)'s SHACL); compile-time exhaustiveness guarantees the match is total.

### Boundaries

- **In scope.** The mapping function, its integration into `compute_params`, its forwarding through the dispatch payload to the worker's `ModelRouter`.
- **Out of scope.** Stakes-driven escalation triggers — [FT-062](FT-062).
- **Out of scope.** Capability `configurable_effort` flag plumbing — [FT-054](FT-054).
- **Out of scope.** Bundle stakes setting — [FT-056](FT-056).
- **Out of scope.** Per-binding effort overrides (intentional: effort is a property of the *stakes*, not the binding; a meta-loop revision of the mapping is a future ADR amendment).

## Out of scope

- Per-capability effort overrides (e.g. "this capability defaults to medium regardless of stakes"). The framework's claim is that stakes is the single per-bundle judgment; effort flows from it.
- A `reasoning_budget` mapping for capabilities that expose a token budget rather than an enum (no such capability in the seed catalog; add a feature_spec if one arrives).
- A measurement TC asserting `reasoning_effort` materially changes model output (PRD §13 lists this as a follow-up experiment, not a launch blocker).
