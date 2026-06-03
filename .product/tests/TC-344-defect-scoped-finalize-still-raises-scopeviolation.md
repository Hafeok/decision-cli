---
id: TC-344
title: defect-scoped finalize still raises ScopeViolation for unrelated code files outside the prior set
type: scenario
status: passing
validates:
  features:
  - FT-137
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --test ft_137_finalize_allowlist defect_scoped_unrelated_code_fails
runner-timeout: 120
observes:
- exit-code
last-run: 2026-06-03T11:44:11.154198982+00:00
last-run-duration: 0.4s
---

## Acceptance criteria

Regression test for the guard itself. Verifies that [FT-137](FT-137)'s expansion of the allowlist does not weaken the guard's protection against worker drift on feature-scoped files outside the prior set and outside the allowlist.

### Conditions

Cargo integration test in `crates/decision-cli/tests/ft_137_finalize_allowlist.rs`. Same harness as TC-343.

**Setup:**

1. Create a tempdir, `git init` it.
2. Stage and commit `crates/foo/src/lib.rs` with message `"[FT-X] Initial implementation"`.
3. Modify the working tree:
   - `crates/foo/src/lib.rs` (modified; in the prior commit — allowed)
   - `crates/bar/src/lib.rs` (newly modified; NOT in the prior commit and NOT in the allowlist)
4. Construct a `FinalizeInput` with `defect_scoped: true`, `feature_id: "FT-X"`, `repo_root: tempdir`, `scope_guard_extras: vec![]`.

**Assertion:**

- `finalize(input)` returns `Err(FinalizeError::ScopeViolation { paths })`.
- `paths` contains `"crates/bar/src/lib.rs"`.
- `paths` does NOT contain `"crates/foo/src/lib.rs"` (in-scope file, must not be flagged).

### Rationale

The point of expanding the allowlist is to permit cross-cutting categories (Cargo.toml, docs, CI). It must NOT silently permit arbitrary code drift. This TC is the negative-case guard that catches a future "I'll just always return true" regression.

### Exit codes

- `0` — finalize returns ScopeViolation listing only `crates/bar/src/lib.rs`.
- `1` — finalize succeeds (guard was inadvertently disabled), OR ScopeViolation lists the wrong paths.

### Surface

`exit-code` — cargo integration test against a tempdir git fixture.