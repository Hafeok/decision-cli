---
id: TC-342
title: scope-guard always-allowed config extras extend the predicate with glob patterns
type: scenario
status: passing
validates:
  features:
  - FT-137
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --lib features::finalize::tests::is_always_allowed_config_extras
runner-timeout: 60
observes:
- exit-code
last-run: 2026-06-03T11:44:11.154198982+00:00
last-run-duration: 0.4s
---

## Acceptance criteria

Verifies that [FT-137](FT-137) §Phase 2's `[scope-guard].always-allowed` config-driven extras correctly augment the default allowlist. Asserts the additive semantic from [ADR-078](ADR-078) Decision §Boundaries.

### Conditions

Unit test in `crates/decision-cli/src/features/finalize/tests.rs`.

**Given extras `["scripts/checks/**", "deny.toml"]`:**

- `is_always_allowed("scripts/checks/foo.sh", &extras)` → `true`
- `is_always_allowed("scripts/checks/nested/bar.sh", &extras)` → `true` (`**` recurses)
- `is_always_allowed("deny.toml", &extras)` → `true`
- `is_always_allowed("Cargo.toml", &extras)` → `true` (defaults still apply)
- `is_always_allowed("scripts/unrelated.sh", &extras)` → `false` (only `scripts/checks/**` is allowed)
- `is_always_allowed("crates/foo/src/lib.rs", &extras)` → `false` (negative case)

**Given empty extras `&[]`:**

- `is_always_allowed("scripts/checks/foo.sh", &[])` → `false` (no extras, no defaults match)
- `is_always_allowed("Cargo.toml", &[])` → `true` (defaults only, still allowed)

### Exit codes

- `0` — all eight assertions hold.
- `1` — at least one fails. Test prints the offending case.

### Surface

`exit-code` — cargo-test runs the predicate as a unit test, no I/O.