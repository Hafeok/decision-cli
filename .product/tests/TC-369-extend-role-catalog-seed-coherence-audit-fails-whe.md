---
id: TC-369
title: extend-role-catalog-seed coherence audit fails when round_trip_tests omits legacy_store_lookup_returns_safe_default
type: scenario
status: passing
validates:
  features:
  - FT-144
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-369-cluster-audit-role-catalog-seed-no-fail-closed-test.sh
runner-timeout: 60
observes:
- exit-code
- stderr
last-run: 2026-06-04T15:47:46.704677747+00:00
last-run-duration: 0.0s
---

## Context

Negative coherence-audit TC for [FT-144](FT-144) — THE DISCRIMINATOR test that proves the `extend-role-catalog-seed` audit locks in [ADR-069](ADR-069)'s fail-closed contract structurally rather than by hope. Audit check #5 asserts: *"`round_trip_tests` has at least one test whose function name matches `legacy_store_lookup_returns_safe_default`."*

This is the load-bearing assertion of FT-144 and the lock-in for FT-121's prototyped invariant: a legacy orchestration store (one written before the new predicate landed) must lookup to a safe default — empty `Vec<String>`, `None`, zero-value — never panic, never crash the caller. The test enforcing this guarantee MUST be present in every cluster output, and the audit MUST fire loudly when it isn't.

Per [ADR-080](ADR-080): *"If [the audit] is weaker than what a single context gave for free, the decomposition is worse than the monolith."* The broad code-writer would have caught a missing fail-closed test only by accident (or not at all — the test's absence is a "did not write" pattern, not a "wrote wrong" pattern, and is invisible to compilation). The audit makes the guarantee structural.

## Setup

- A fixture directory under `tests/fixtures/cluster-audit-extend-role-catalog-seed/no-fail-closed-test/` identical to the positive fixture (TC-367) EXCEPT:
  - `round_trip_tests.rs` contains the three OTHER tests (seed→lookup→assert, SHACL-passes-on-seeded, SHACL-fails-on-malformed) but DOES NOT contain any test function whose name matches `legacy_store_lookup_returns_safe_default`.
- All other cells (iri_constants, seed_quad_function, init_pipeline_wiring, shacl_shape_extension, role_struct_field_extension) are internally consistent with the positive fixture so the audit's only failure signal is the missing fail-closed test.
- The wrapper script `scripts/checks/tc-369-cluster-audit-role-catalog-seed-no-fail-closed-test.sh` invokes the audit with all six paths and `--params '{"requires_shacl": true, "surfaces_on_role_struct": true}'`, capturing exit code + stderr.

## Steps

1. Run `scripts/checks/cluster-audit-extend-role-catalog-seed.py` against the no-fail-closed-test fixture via the wrapper.
2. Capture exit code and stderr.

## Expected outcome

- Exit code 1 (audit failure, not unrunnable).
- Stderr contains a FAIL line for check #5 — substring match: `FAIL fail_closed_default_test_present` (or the canonical check-5 identifier as named by the audit script).
- The FAIL detail names ADR-069 and the expected test name `legacy_store_lookup_returns_safe_default` so the operator can map the failure to its contract without consulting the audit source: substring match on `legacy_store_lookup_returns_safe_default` AND on `ADR-069`.
- Other checks (1..4, 6) PASS because the rest of the cluster is healthy.

## Pass / fail

- Pass: bash runner exits 0 because the wrapper asserts the audit exited 1 AND stderr matched the check-5 identifier AND the `legacy_store_lookup_returns_safe_default` substring AND the `ADR-069` substring.
- Fail: the audit script unexpectedly exits 0 (audit has no teeth for missing-fail-closed-test — the ADR-069 guarantee can silently be dropped by any consuming cluster) OR exits 2 (unrunnable) OR exits 1 without naming check #5 / the canonical test name / ADR-069.

## Why this is the load-bearing TC

ADR-069's fail-closed guarantee is the kind of contract that, once dropped, only surfaces in production when a legacy store hits a freshly-deployed binary and the missing predicate panics a hot path. FT-121 prototyped the test convention; FT-144's audit makes the convention structural. Without this TC, an LLM-generated `round_trip_tests` output could quietly omit the fail-closed test, the cluster would commit, and the next consumer of `extend-role-catalog-seed` would discover the regression on a customer's machine. This test is the safety net that says: *"the audit's check #5 catches the omission before the worktree merges, every time, by name."*