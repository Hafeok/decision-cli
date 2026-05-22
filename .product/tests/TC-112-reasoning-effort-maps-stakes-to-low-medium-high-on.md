---
id: TC-112
title: Reasoning_effort maps stakes to low/medium/high only when configurable_effort is true
type: exit-criteria
status: unimplemented
validates:
  features:
  - FT-063
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test reasoning_effort_mapping
runner-timeout: 120
---

## Description

Invariant (PRD §11.2 bullets 4–5): `core::dispatcher::params::compute_reasoning_effort` maps `Bundle.stakes` to `reasoning_effort` exactly per the [ADR-035](ADR-035) table, and *only* when the resolved capability has `configurable_effort = true`. The architect role dispatched with `stakes = routine` produces a `standard-reasoning` call with `reasoning_effort = low`; `stakes = elevated` triggers escalation to `standard-reasoning-frontier` (which is *not* configurable, so `reasoning_effort` is absent on that call).

The runner is `cargo-test`.

Acceptance:

1. **Pure function — all 6 cases.**
   - `compute_reasoning_effort(Routine, true) == Some("low")`
   - `compute_reasoning_effort(Elevated, true) == Some("medium")`
   - `compute_reasoning_effort(Foundational, true) == Some("high")`
   - `compute_reasoning_effort(Routine, false) == None`
   - `compute_reasoning_effort(Elevated, false) == None`
   - `compute_reasoning_effort(Foundational, false) == None`
2. **Architect routine path (PRD §11.2 bullet 4).** Seed catalog. Stub architect worker to record `CallParams.reasoning_effort`. Dispatch architect with `stakes = "routine"`. Assert exactly one session; capability is `standard-reasoning`; the captured `CallParams.reasoning_effort == Some("low")`.
3. **Architect elevated path (PRD §11.2 bullet 5).** Same as above but `stakes = "elevated"`. Per architect's seed binding, the `stakes_elevated` trigger fires on the first escalation step (`standard-reasoning-frontier`). Assert two sessions:
   - S1 (`standard-reasoning`): `CallParams.reasoning_effort == Some("medium")`.
   - S2 (`standard-reasoning-frontier`): `CallParams.reasoning_effort == None` (its `configurable_effort` is false).
4. **Anthropic ignores reasoning_effort.** Force a deep-reasoning escalation (architect with foundational stakes + low-confidence stubs). Assert S3's call to the Anthropic router is made *without* a `reasoning_effort` parameter present in the API request (the router skips it when `configurable_effort = false`).
5. **Exhaustiveness.** The Rust `match` in `compute_reasoning_effort` covers every `Stakes` variant; adding a variant without updating the function fails to compile (this is enforced by Rust's exhaustiveness check, the test does not need to assert it dynamically — but the test file documents the expectation).

⟦Σ:Types⟧{
  Effort ≜ low | medium | high
  Mapping: Stakes × Bool → Maybe Effort
}

⟦Γ:Invariants⟧{
  compute_reasoning_effort(_, false) = None
  compute_reasoning_effort(Routine, true) = Some(low)
  compute_reasoning_effort(Elevated, true) = Some(medium)
  compute_reasoning_effort(Foundational, true) = Some(high)
}
