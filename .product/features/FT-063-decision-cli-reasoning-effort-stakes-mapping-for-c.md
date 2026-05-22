---
id: FT-063
title: 'decision-cli: reasoning_effort stakes mapping for configurable_effort capabilities'
phase: 2
status: planned
depends-on:
- FT-056
- FT-061
adrs:
- ADR-001
- ADR-002
- ADR-004
- ADR-005
- ADR-008
- ADR-012
- ADR-013
- ADR-014
- ADR-015
- ADR-016
- ADR-017
- ADR-018
- ADR-020
- ADR-021
- ADR-022
- ADR-023
- ADR-024
- ADR-025
- ADR-027
- ADR-033
- ADR-034
- ADR-035
- ADR-036
- ADR-037
tests:

- TC-112
domains:
- api
domains-acknowledged: {}
---

## Description

When the dispatcher resolves a role to a capability with `configurable_effort = true` (e.g. `standard-reasoning` / gpt-oss-120b per [FT-054](FT-054)), compute the `reasoning_effort` model parameter from the bundle's `stakes` field per [ADR-035](ADR-035) and inject it into the dispatch payload's `parameters.reasoning_effort`. The mapping is fixed: `routine → low`, `elevated → medium`, `foundational → high`. For capabilities without `configurable_effort`, the dispatcher leaves `reasoning_effort` null and the worker ignores it.

Per PRD §14 resolution, the Scaleway API accepts `reasoning_effort` as a **top-level kwarg** on the standard `chat.completions.create` call (not via `extra_body`). Valid values: `'none'`, `'low'`, `'medium'`, `'high'`. The `'none'` value is reserved in the dispatcher vocabulary but not currently bound to any stakes level — available for explicit "skip reasoning entirely" cases if a future binding needs it.

This is the smallest of the new features but the most concrete demonstration of why the per-bundle stakes judgment from [FT-056](FT-056) and the per-capability properties from [FT-054](FT-054) work together: a single field on the bundle drives a model parameter on a specific capability without code branches in the worker.

## Functional Specification

### Inputs

- `ResolvedCapability` from [FT-061](FT-061) with `configurable_effort: bool`.
- `Bundle.stakes` from [FT-056](FT-056).
- The dispatch payload assembly point in `core::dispatcher::compute_params` ([FT-061](FT-061)).

### Outputs

- New type and function in `core::dispatcher::params`:
  ```rust
  #[derive(Copy, Clone, Debug, PartialEq, Eq)]
  pub enum ReasoningEffort { None_, Low, Medium, High }
  
  impl ReasoningEffort {
      pub fn as_str(self) -> &'static str {
          match self {
              ReasoningEffort::None_ => "none",
              ReasoningEffort::Low => "low",
              ReasoningEffort::Medium => "medium",
              ReasoningEffort::High => "high",
          }
      }
  }
  
  pub fn compute_reasoning_effort(stakes: Stakes, configurable_effort: bool) -> Option<ReasoningEffort> {
      if !configurable_effort { return None; }
      Some(match stakes {
          Stakes::Routine => ReasoningEffort::Low,
          Stakes::Elevated => ReasoningEffort::Medium,
          Stakes::Foundational => ReasoningEffort::High,
      })
  }
  ```
- Dispatch payload's `parameters.reasoning_effort` populated from this function (the enum's `as_str()` value). Where the capability is not `configurable_effort`, the field is absent / `null`.
- The Scaleway client wrapper from [FT-059](FT-059) consumes `reasoning_effort` from `params` and passes it as a **top-level kwarg** in `client.chat.completions.create(..., reasoning_effort=...)`. Verified against the live Scaleway API per PRD §14; no `extra_body` workaround needed.

### State

- No state. Pure function plus a payload-assembly call.

### Behaviour

1. After [FT-061](FT-061) resolves the capability, `compute_params` is called.
2. `compute_params` invokes `compute_reasoning_effort(bundle.stakes, resolved.configurable_effort)`.
3. The result (`Option<ReasoningEffort>`) is serialised to the dispatch payload as a string-or-null value at `params.reasoning_effort`.
4. The worker's `ModelRouter` (from [FT-060](FT-060)) forwards the value to the API call as a top-level kwarg.
5. For Anthropic capabilities (which do not have `reasoning_effort`), `configurable_effort` is false and the field is `null` — the Anthropic router ignores it.

### Mapping table

The mapping is a fixed property of the framework, not per-binding policy:

| `bundle.stakes` | `reasoning_effort` | Note |
|---|---|---|
| `routine` | `low` | default for most dispatches |
| `elevated` | `medium` | mid-tier reasoning depth |
| `foundational` | `high` | maximum reasoning depth |
| (reserved) | `none` | not bound to any stakes; available in vocabulary for explicit "skip reasoning" cases |

Note: For the architect role binding, `stakes = foundational` *also* triggers escalation to `standard-reasoning-frontier` per [FT-058](FT-058)'s seed bindings. In practice the default capability (`standard-reasoning`) sees `routine` and `elevated`; `foundational` reaches it only if a later operator removes the escalation step. The `foundational → high` mapping is preserved for completeness.

The `'none'` value is reserved but not produced by `compute_reasoning_effort` from any current stakes input. It exists so a future binding (or a meta-loop revision) can construct a payload with `reasoning_effort = "none"` directly — e.g. for a "fast classification" dispatch where reasoning is undesired even on a `configurable_effort` model. Until such a binding exists, the dispatcher does not emit `none` from this function.

### Invariants

- `compute_reasoning_effort(_, false) == None` for every input. The function does not produce a value when the capability does not accept it.
- The function is total over `Stakes`: every variant maps to a defined value.
- The function is referentially transparent — same inputs always yield same output, no graph reads.
- The output string at the dispatch payload is exactly one of `{"none", "low", "medium", "high"}` or absent; Scaleway's Pydantic validator on the server side rejects anything else.

### Error handling

- None at the function level — the function cannot fail. An unknown stakes value is impossible (the enum is closed via [FT-056](FT-056)'s SHACL); compile-time exhaustiveness guarantees the match is total.
- An invalid value reaching Scaleway (impossible if the dispatcher uses this function, possible if a future override bypasses it) → Scaleway returns 400 with a Pydantic error; [FT-059](FT-059)'s wrapper surfaces this as `ScalewayClientError(category="invalid_params")`.

### Boundaries

- **In scope.** The mapping function, its integration into `compute_params`, its forwarding through the dispatch payload to the worker's `ModelRouter`.
- **Out of scope.** Stakes-driven escalation triggers — [FT-062](FT-062).
- **Out of scope.** Capability `configurable_effort` flag plumbing — [FT-054](FT-054).
- **Out of scope.** Bundle stakes setting — [FT-056](FT-056).
- **Out of scope.** Per-binding effort overrides (intentional: effort is a property of the *stakes*, not the binding; a meta-loop revision of the mapping is a future ADR amendment).

## Out of scope

- Per-capability effort overrides (e.g. "this capability defaults to medium regardless of stakes"). The framework's claim is that stakes is the single per-bundle judgment; effort flows from it.
- Producing `none` from a stakes input (the `none` value is reserved in the enum / vocabulary but not currently bound to any stakes level).
- A `reasoning_budget` mapping for capabilities that expose a token budget rather than an enum (no such capability in the seed catalog; add a feature_spec if one arrives).
- A measurement TC asserting `reasoning_effort` materially changes model output (PRD §13 lists this as a follow-up experiment; PRD §14 confirms token counts modulate empirically — `low=22`, baseline `=45`, `high=149` completion tokens on a fixed prompt — but quality measurement is a follow-up A/B test against `mid-reasoning` / `fast-reasoning` candidates, not a launch blocker).
