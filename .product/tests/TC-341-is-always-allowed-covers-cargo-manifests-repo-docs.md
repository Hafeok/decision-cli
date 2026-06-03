---
id: TC-341
title: is_always_allowed covers Cargo manifests, repo docs, CI configs, and VCS metadata by default
type: exit-criteria
status: passing
validates:
  features:
  - FT-137
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --lib features::finalize::tests::is_always_allowed_default_categories
runner-timeout: 60
observes:
- exit-code
last-run: 2026-06-03T11:44:11.154198982+00:00
last-run-duration: 0.4s
---

## Acceptance criteria

Verifies that [FT-137](FT-137)'s expanded `is_always_allowed` predicate (replacing the old `is_system_path`) covers the four new default categories from [ADR-078](ADR-078).

### Conditions

Unit test in `crates/decision-cli/src/features/finalize/tests.rs`. For each category, the predicate returns `true` on the listed paths and `false` on a representative non-allowlisted code path. `extras` argument is an empty slice (`&[]`) for this test — defaults only.

**Positive cases (must return `true`):**

- Build manifests: `Cargo.toml`, `crates/foo/Cargo.toml`, `crates/decision-cli/Cargo.toml`, `Cargo.lock`, `pyproject.toml`, `uv.lock`, `package.json`, `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`.
- Repo-level docs (root only): `CLAUDE.md`, `README.md`, `CONTRIBUTING.md`, `LICENSE`, `LICENSE.md`, `LICENSE.txt`, `CODE_OF_CONDUCT.md`, `CHANGELOG.md`.
- CI / packaging: `.github/workflows/release.yml`, `.github/CODEOWNERS`, `.cargo/config.toml`, `dist-workspace.toml`, `rust-toolchain.toml`, `rust-toolchain`.
- VCS metadata: `.gitignore`, `.gitattributes`.
- Existing prefixes (regression guard): `.product/features/FT-001.md`, `.dec/store/orchestration.nq`.

**Negative cases (must return `false`):**

- `crates/decision-cli/src/main.rs`
- `crates/decision-cli/tests/integration.rs`
- `docs/architecture.md`
- `workers/code-writer/main.py`
- `crates/foo/src/lib.rs`

### Exit codes

- `0` — every positive case returns `true` AND every negative case returns `false`.
- `1` — at least one case fails. The test prints which path produced the wrong verdict.

### Surface

`exit-code` — cargo-test runs the predicate as a pure unit test, no I/O.