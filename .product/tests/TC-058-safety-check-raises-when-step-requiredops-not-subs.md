---
id: TC-058
title: Safety check raises when step requiredOps not subset of env allowedOps
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test tc_058_safety_check_raises_when_step_requiredops_not_subs
runner-timeout: 120
last-run: 2026-05-24T19:13:54.524063922+00:00
last-run-duration: 0.2s
---

## Description

[FT-037](FT-037)'s `check_graph_against_env` and `check_step_against_env` enforce `step.requiredOps ⊆ env.allowedOps` per [ADR-028](ADR-028) §Safety gating. This TC exercises the violation path: a graph or step whose required ops escape the env's allowed set is rejected with `Error::SafetyViolation` carrying full diagnostic context.

## Acceptance Criteria

1. **Single-step violation.** Given env `prod-readonly` (`dec:allowedOps ("http-readonly")`) and a step of kind `http-request` with method `POST` (`requiredOps = {http-mutating}`), `check_step_against_env` returns `Err(SafetyViolation { missing_ops: ["http-mutating"], env_safety_class: ProductionReadonly, ... })`.

2. **Whole-graph violation finds first.** Given a graph with three steps where step 2 violates safety, `check_graph_against_env` returns the violation on step 2 and the diagnostic identifies it by step IRI.

3. **All-violations variant.** `check_graph_against_env_all` returns every violation in step order — a graph with two violating steps surfaces both.

4. **Op direction.** A step requiring `{shell}` against an env allowing `{shell, filesystem}` passes (subset). A step requiring `{shell, filesystem}` against an env allowing `{shell}` fails with `missing_ops: ["filesystem"]`.

5. **Conditional ops for sparql-assertion.** A `sparql-assertion` step with `dec:target ".dec/store"` requires `sparql-local`; against env allowing only `sparql-http`, it fails. Same step with `dec:target "https://example.com/sparql"` requires `sparql-http`; against env allowing only `sparql-local`, it fails.

6. **Unknown op token.** A step kind declaring an unrecognised op or an env declaring an unknown op surfaces `Error::UnknownOp { token, source }` rather than a silent pass.

## Fixture

- A `core::verify::safety` unit-test module.
- Programmatically constructed `VerificationGraph` and `VerificationEnvironment` values; no on-disk I/O.

## Out of scope

- StreamWriter integration (TC-059).
- Composition with autonomy levels (slice 3).