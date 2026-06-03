---
id: TC-343
title: defect-scoped finalize succeeds when worker touches Cargo.toml plus a prior-set file
type: scenario
status: passing
validates:
  features:
  - FT-137
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --test ft_137_finalize_allowlist defect_scoped_cargo_toml_succeeds
runner-timeout: 120
observes:
- exit-code
last-run: 2026-06-03T11:44:11.154198982+00:00
last-run-duration: 0.5s
---

## Acceptance criteria

Integration test for [FT-137](FT-137)'s end-to-end behaviour. Reproduces the [FT-136](FT-136) misfire pattern and asserts that finalize now succeeds.

### Conditions

Cargo integration test in `crates/decision-cli/tests/ft_137_finalize_allowlist.rs` (or similar). Uses a tempdir-backed git repo fixture.

**Setup:**

1. Create a tempdir, `git init` it.
2. Stage and commit `crates/foo/src/lib.rs` with message `"[FT-X] Initial implementation"`.
3. Modify the working tree:
   - `Cargo.toml` (newly modified; not in the prior commit)
   - `crates/foo/src/lib.rs` (modified; in the prior commit)
   - `CLAUDE.md` (newly modified; not in the prior commit)
4. Construct a `FinalizeInput` with `defect_scoped: true`, `feature_id: "FT-X"`, `repo_root: tempdir`, `scope_guard_extras: vec![]`.

**Assertion:**

- `finalize(input)` returns `Ok(_)`.
- The commit lands successfully.
- No `FinalizeError::ScopeViolation` is produced.

### Rationale

Without this slice, the iteration above would fail with `ScopeViolation` listing `Cargo.toml` and `CLAUDE.md` as out-of-scope. With [ADR-078](ADR-078)'s default allowlist, both are project-wide categories and pass.

### Exit codes

- `0` — finalize succeeds with no ScopeViolation.
- `1` — finalize returns ScopeViolation or any other error. Test prints the error.

### Surface

`exit-code` — cargo integration test against a tempdir git fixture.