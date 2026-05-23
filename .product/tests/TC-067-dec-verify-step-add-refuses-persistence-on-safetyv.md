---
id: TC-067
title: dec verify step add refuses persistence on SafetyViolation
type: scenario
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test tc_067_dec_verify_step_add_refuses_persistence_on_safetyv
runner-timeout: 120
last-run: 2026-05-23T17:59:55.770418918+00:00
last-run-duration: 0.4s
---

## Description

[FT-044](FT-044) integrates [FT-037](FT-037)'s safety check on every step append per [ADR-028](ADR-028) §Safety gating. This TC asserts the integration: a step whose `requiredOps` escape the graph's env's `allowedOps` is rejected before any on-disk write or store mutation.

## Acceptance Criteria

1. **Violation path.** Given env `prod-readonly` with `dec:allowedOps ("http-readonly")` and a graph `VG-prod` referencing it, `dec verify step add VG-prod --type http-request --field method=POST --field url=https://example.com --field expect-status=200` exits 1 with `Error::SafetyViolation`. The stderr diagnostic names the step kind, the missing op (`http-mutating`), the env id, the env's safety class, and the env's `allowedOps` list.

2. **No on-disk side effect.** After the failing call in (1), the on-disk `.ttl` for `VG-prod` is byte-identical to its pre-call state. No temp file (`*.tmp`) remains.

3. **No store mutation.** After (1), a SPARQL count of `dec:VerificationStep` quads in the orchestration store is unchanged from before the call.

4. **MCP surfaces same error.** `dec_verify_step_add` with the same input returns the structured `SafetyViolation` error with identical fields (step_id, step_kind, missing_ops, env_id, env_allowed_ops, env_safety_class).

5. **Subsequent allowed step still works.** After the failing call, appending a benign `--type shell-command` step to a *different* graph in an *isolated* env succeeds — the failed call did not poison subsequent state.

6. **First-violation diagnostic.** When `dec verify step add` is invoked with multiple field errors plus a safety violation, the safety violation surfaces (safety runs before SHACL per [FT-037](FT-037) §Behaviour).

## Fixture

- Tempdir with at least two envs covering both `isolated` and `production-readonly` safety classes, and one graph per env.

## Out of scope

- Pure safety-check unit tests (TC-058).
- StreamWriter chokepoint structural test (TC-059).
- Slice-3 runtime safety during execution.